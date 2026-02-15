# Spotify Search Service

A Rust backend that searches the Spotify API for songs.

## Prerequisites

1. **Spotify Developer Account**: Create an app at [Spotify Developer Dashboard](https://developer.spotify.com/dashboard) to get:
   - Client ID
   - Client Secret

2. **Rust** (for local dev) or **Docker**

## Quick Start

### Run with Docker

```bash
docker build -t spotify-search .
docker run -p 8081:8081 \
  -e SPOTIFY_CLIENT_ID=your_client_id \
  -e SPOTIFY_CLIENT_SECRET=your_client_secret \
  spotify-search
```

### Run locally

```bash
export SPOTIFY_CLIENT_ID=your_client_id
export SPOTIFY_CLIENT_SECRET=your_client_secret
cargo run
```

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |
| GET | `/api/v1/search` | Search Spotify for tracks |
| GET | `/api/v1/search?include_features=true` | Search with audio features + embeddings |
| GET | `/api/v1/tracks/with-features` | Get tracks by IDs with embeddings (called by Go saga) |

### Search

```bash
curl "http://localhost:8081/api/v1/search?q=blinding+lights&limit=10"
```

**Query params:**
- `q` (required): Search query (artist, track, album, etc.)
- `limit` (optional): 1–50, default 20
- `offset` (optional): Pagination offset, 0–1000
- `include_features` (optional): If true, adds `embedding` (12-dim from Spotify audio features) and `metadata` per track

### Tracks with features (for Go saga)

```bash
curl "http://localhost:8081/api/v1/tracks/with-features?ids=0VjIjW4GlUZAMYd2vXMi3b,5QO79kh1waicV47BqGRL3g"
```

**Example response:**
```json
{
  "tracks": [
    {
      "id": "...",
      "name": "Blinding Lights",
      "uri": "spotify:track:...",
      "duration_ms": 200040,
      "explicit": false,
      "artists": [{"id": "...", "name": "The Weeknd"}],
      "album": {"id": "...", "name": "After Hours", "image_url": "https://..."},
      "spotify_url": "https://open.spotify.com/track/..."
    }
  ],
  "total": 1234,
  "limit": 10,
  "offset": 0
}
```

## Configuration

| Env Var | Required | Default | Description |
|---------|----------|---------|-------------|
| `SPOTIFY_CLIENT_ID` | Yes | - | Spotify app Client ID |
| `SPOTIFY_CLIENT_SECRET` | Yes | - | Spotify app Client Secret |
| `PORT` | No | 8081 | HTTP port |
| `GRPC_PORT` | No | 50051 | gRPC port (for Go service) |

## Authentication

Uses Spotify **Client Credentials** flow (server-to-server). No user OAuth— suitable for catalog search. Tokens are cached and refreshed automatically.
# spotify-search
