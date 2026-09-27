# m3u8-to-http-proxy

Lightweight HTTP audio proxy written in Rust that converts live HLS (`.m3u8`) radio streams into continuous, chunked HTTP audio streams (`audio/aac` or `audio/mpeg`).

---

## Why this exists

Modern online radio stations and live broadcast networks frequently stream exclusively via HTTP Live Streaming (HLS) playlists containing rotating segment files.

Many audio engines, Discord music nodes (such as NodeLink and Lavalink), and media players only accept continuous byte streams (like Icecast/Shoutcast) and reject `application/x-mpegURL` playlists because they lack built-in HLS demuxers for generic HTTP sources.

`m3u8-to-http-proxy` sits between the HLS broadcaster and your audio consumer:

```
Broadcaster (HLS CDN)
   └─> .m3u8 playlist + rotating .aac/.mp3 segments
          │
          ▼
   m3u8-to-http-proxy (Rust / Axum / Tokio)
   - Polls rolling manifest based on target duration
   - Streams newly published chunks sequentially
   - Drops polling task immediately when client disconnects
          │
          ▼  HTTP chunked audio (audio/aac or audio/mpeg)
   NodeLink / Lavalink / VLC / Audio Client
```

---

## Features

- **Key-Value Station Routing**: Map station keys to upstream `.m3u8` URLs in `config.toml`. Access streams dynamically via `/radio/:key`.
- **Zero Re-encoding**: Passes through raw audio frames (ADTS AAC / MP3) directly from segments without transcoding overhead or quality loss.
- **Resource Footprint**: Uses ~10 MB RAM and near-zero CPU under Tokio's asynchronous I/O runtime.
- **Automatic Lifecycle Management**: Segment polling loops are tied to the client connection's lifecycle. Dropping the connection instantly stops upstream requests.
- **Config Fallback**: Automatically loads `config.toml` or falls back to `config.example.toml` if no custom path is provided.

---

## Configuration

Create a `config.toml` file in the working directory (or copy from `config.example.toml`):

```toml
[server]
host = "0.0.0.0"
port = 3050

[stations]
chill = "https://example.com/hls/chill/playlist.m3u8"
news = "https://example.com/hls/news/playlist.m3u8"
```

To specify a custom config file path at runtime:

```bash
./m3u8-to-http-proxy /path/to/my-config.toml
```

---

## Endpoints

| Method | Path | Description |
| :--- | :--- | :--- |
| `GET` | `/radio/:key` | Continuous chunked audio stream for the configured station key |
| `GET` | `/stream/:key` | Alias for `/radio/:key` |
| `GET` | `/stations` | JSON response listing all configured station keys |
| `GET` | `/health` | Health check endpoint (returns `200 OK`) |

### Example Request

```bash
# Stream audio directly via curl or VLC
curl -N http://localhost:3050/radio/chill > stream.aac

# Play directly in ffplay / VLC
ffplay http://localhost:3050/radio/chill
```

---

## Building and Running

### Prerequisites

- Rust 1.75+ (`cargo`)

### Build from source

```bash
# Debug build
cargo build

# Optimized release binary
cargo build --release
```

The resulting binary will be at `target/release/m3u8-to-http-proxy`.

### Running with Cargo

```bash
cargo run --release
```

---

## Audio Engine & Bot Integration (e.g. NodeLink / Lavalink)

In your audio client or Discord bot configuration, point the stream URL to the proxy endpoint:

```typescript
export const RADIO_STATIONS = {
  chill: {
    id: "chill",
    name: "Chillout Lounge",
    streamUrl: "http://127.0.0.1:3050/radio/chill",
  },
};
```

When hosted alongside NodeLink/Lavalink on the same server, playback connects via loopback (`127.0.0.1`) with sub-millisecond latency.

---

## License

MIT
