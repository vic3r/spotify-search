//! HTTP handlers for the Spotify search API.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::spotify::{SpotifyClient, Track};

/// Query parameters for search endpoint.
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// Search query (required).
    pub q: String,
    /// Max results (1-50, default 20).
    #[serde(default)]
    pub limit: Option<u32>,
    /// Pagination offset (0-1000).
    #[serde(default)]
    pub offset: Option<u32>,
}

/// API response for track search.
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub tracks: Vec<TrackResponse>,
    pub total: u32,
    pub limit: u32,
    pub offset: u32,
}

/// Single track in API response.
#[derive(Debug, Serialize)]
pub struct TrackResponse {
    pub id: String,
    pub name: String,
    pub uri: String,
    pub duration_ms: u32,
    pub explicit: bool,
    pub artists: Vec<ArtistResponse>,
    pub album: AlbumResponse,
    pub spotify_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ArtistResponse {
    pub id: Option<String>,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct AlbumResponse {
    pub id: Option<String>,
    pub name: String,
    pub image_url: Option<String>,
}

fn track_to_response(t: &Track) -> TrackResponse {
    TrackResponse {
        id: t.id.clone(),
        name: t.name.clone(),
        uri: t.uri.clone(),
        duration_ms: t.duration_ms,
        explicit: t.explicit,
        artists: t.artists.iter().map(|a| ArtistResponse {
            id: a.id.clone(),
            name: a.name.clone(),
        }).collect(),
        album: AlbumResponse {
            id: t.album.id.clone(),
            name: t.album.name.clone(),
            image_url: t.album.images.first().and_then(|i| i.url.clone()),
        },
        spotify_url: t.external_urls.spotify.clone(),
    }
}

/// GET /health - Health check.
pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

/// GET /api/v1/search - Search Spotify for tracks.
pub async fn search(
    State(spotify): State<SpotifyClient>,
    Query(params): Query<SearchQuery>,
) -> Result<impl IntoResponse, AppError> {
    if params.q.trim().is_empty() {
        return Err(AppError::BadRequest("query 'q' is required and cannot be empty".into()));
    }

    let result = spotify
        .search_tracks(&params.q, params.limit, params.offset)
        .await
        .map_err(|e| AppError::Spotify(e))?;

    let response = SearchResponse {
        tracks: result.tracks.iter().map(track_to_response).collect(),
        total: result.total,
        limit: result.limit,
        offset: result.offset,
    };

    Ok((StatusCode::OK, Json(response)))
}

/// Build the API router.
pub fn router() -> Router<SpotifyClient> {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/search", get(search))
}
