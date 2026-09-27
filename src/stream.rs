use bytes::Bytes;
use futures_util::Stream;
use std::time::Duration;
use tracing::{debug, error, warn};

pub fn resolve_segment_url(base_manifest_url: &str, segment_line: &str) -> String {
    if segment_line.starts_with("http://") || segment_line.starts_with("https://") {
        segment_line.to_string()
    } else if segment_line.starts_with('/') {
        if let Ok(parsed) = reqwest::Url::parse(base_manifest_url) {
            let origin = format!(
                "{}://{}",
                parsed.scheme(),
                parsed.host_str().unwrap_or_default()
            );
            format!("{}{}", origin, segment_line)
        } else {
            segment_line.to_string()
        }
    } else {
        match base_manifest_url.rfind('/') {
            Some(idx) => format!("{}{}", &base_manifest_url[..=idx], segment_line),
            None => segment_line.to_string(),
        }
    }
}

pub fn create_hls_stream(
    client: reqwest::Client,
    manifest_url: String,
    station_key: String,
) -> (
    &'static str,
    impl Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static,
) {
    // Detect audio MIME type from URL if possible
    let content_type = if manifest_url.contains(".mp3") {
        "audio/mpeg"
    } else {
        "audio/aac"
    };

    let stream = async_stream::stream! {
        let mut last_seq: i64 = -1;
        let mut poll_interval = Duration::from_millis(2500);

        debug!(station = %station_key, url = %manifest_url, "Starting HLS consumer loop");

        loop {
            match client.get(&manifest_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.text().await {
                        Ok(manifest) => {
                            let lines: Vec<&str> = manifest.lines().map(str::trim).collect();

                            if let Some(target_dur) = lines.iter()
                                .find(|l| l.starts_with("#EXT-X-TARGETDURATION:"))
                                .and_then(|l| l.split(':').nth(1))
                                .and_then(|s| s.parse::<f64>().ok())
                            {
                                let ms = (target_dur * 1000.0 * 0.65).clamp(1200.0, 4000.0) as u64;
                                poll_interval = Duration::from_millis(ms);
                            }

                            let base_seq = lines.iter()
                                .find(|l| l.starts_with("#EXT-X-MEDIA-SEQUENCE:"))
                                .and_then(|l| l.split(':').nth(1))
                                .and_then(|s| s.parse::<i64>().ok())
                                .unwrap_or(0);

                            let segment_lines: Vec<&str> = lines.into_iter()
                                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                                .collect();

                            let total_segments = segment_lines.len();

                            // On initial connection, fast-forward to the latest 2 segments
                            if last_seq < 0 && total_segments > 2 {
                                last_seq = base_seq + (total_segments as i64) - 2;
                            }

                            for (idx, seg_rel_url) in segment_lines.into_iter().enumerate() {
                                let current_seq = base_seq + (idx as i64);
                                if current_seq > last_seq {
                                    let full_url = resolve_segment_url(&manifest_url, seg_rel_url);
                                    match client.get(&full_url).send().await {
                                        Ok(seg_resp) if seg_resp.status().is_success() => {
                                            match seg_resp.bytes().await {
                                                Ok(bytes) => {
                                                    last_seq = current_seq;
                                                    yield Ok::<Bytes, std::io::Error>(bytes);
                                                }
                                                Err(e) => {
                                                    warn!(station = %station_key, seq = current_seq, error = %e, "Failed to read segment payload");
                                                }
                                            }
                                        }
                                        Ok(seg_resp) => {
                                            warn!(station = %station_key, status = %seg_resp.status(), "Segment request non-200");
                                        }
                                        Err(e) => {
                                            warn!(station = %station_key, error = %e, "Segment fetch network error");
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!(station = %station_key, error = %e, "Failed to decode HLS manifest text");
                        }
                    }
                }
                Ok(resp) => {
                    warn!(station = %station_key, status = %resp.status(), "Manifest fetch returned non-200");
                }
                Err(e) => {
                    error!(station = %station_key, error = %e, "Manifest fetch network error");
                }
            }

            tokio::time::sleep(poll_interval).await;
        }
    };

    (content_type, stream)
}
