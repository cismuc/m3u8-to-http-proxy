mod config;
mod stream;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use config::AppConfig;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

struct AppState {
    client: reqwest::Client,
    stations: HashMap<String, String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "m3u8_to_http_proxy=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let custom_config_path = std::env::args().nth(1);
    let config = AppConfig::load(custom_config_path.as_deref())?;

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("M3u8ToHttpProxy/1.0 (https://github.com/cismuc/m3u8-to-http-proxy)")
        .build()?;

    let state = Arc::new(AppState {
        client: http_client,
        stations: config.stations,
    });

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/stations", get(list_stations))
        .route("/radio/:key", get(handle_stream))
        .route("/stream/:key", get(handle_stream))
        .with_state(state);

    let bind_addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    info!("M3u8ToHttpProxy listening on http://{}", bind_addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn health_check() -> &'static str {
    "OK"
}

async fn list_stations(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let keys: Vec<String> = state.stations.keys().cloned().collect();
    Json(serde_json::json!({
        "status": "ok",
        "count": keys.len(),
        "stations": keys,
    }))
}

async fn handle_stream(
    Path(key): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let manifest_url = match state.stations.get(&key) {
        Some(url) => url.clone(),
        None => {
            warn!(station = %key, "Station key not found in configuration");
            return (
                StatusCode::NOT_FOUND,
                format!("Station '{}' not found in configuration\n", key),
            )
                .into_response();
        }
    };

    info!(station = %key, url = %manifest_url, "Serving continuous audio stream");

    let (content_type, body_stream) = stream::create_hls_stream(
        state.client.clone(),
        manifest_url,
        key,
    );

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, content_type.parse().unwrap());
    headers.insert(
        header::CACHE_CONTROL,
        "no-cache, no-store, must-revalidate".parse().unwrap(),
    );
    headers.insert(header::CONNECTION, "keep-alive".parse().unwrap());

    (headers, Body::from_stream(body_stream)).into_response()
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("Shutdown signal received (Ctrl+C)"),
        _ = terminate => info!("Shutdown signal received (SIGTERM)"),
    }
}
