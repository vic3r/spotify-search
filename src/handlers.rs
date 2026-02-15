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
use crate::spotify::{SpotifyClient, Track, TrackWithFeatures};

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
    /// Include audio features and embeddings in response (for Go import).
    #[serde(default)]
    pub include_features: Option<bool>,
}

/// Query parameters for GET tracks with features (called by Go saga).
#[derive(Debug, Deserialize)]
pub struct TracksWithFeaturesQuery {
    /// Comma-separated Spotify track IDs (max 50).
    pub ids: String,
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
    /// 12-dim embedding from Spotify audio features (when include_features=true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    /// Metadata for Go import (spotify_id, title, artist, album).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, String>>,
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
    track_with_features_to_response(&TrackWithFeatures {
        track: t.clone(),
        audio_features: None,
        embedding: None,
    })
}

fn track_with_features_to_response(t: &TrackWithFeatures) -> TrackResponse {
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("spotify_id".into(), t.track.id.clone());
    metadata.insert("title".into(), t.track.name.clone());
    metadata.insert("artist".into(), t.track.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", "));
    metadata.insert("album".into(), t.track.album.name.clone());
    if let Some(ref url) = t.track.external_urls.spotify {
        metadata.insert("spotify_url".into(), url.clone());
    }

    TrackResponse {
        id: t.track.id.clone(),
        name: t.track.name.clone(),
        uri: t.track.uri.clone(),
        duration_ms: t.track.duration_ms,
        explicit: t.track.explicit,
        artists: t.track.artists.iter().map(|a| ArtistResponse {
            id: a.id.clone(),
            name: a.name.clone(),
        }).collect(),
        album: AlbumResponse {
            id: t.track.album.id.clone(),
            name: t.track.album.name.clone(),
            image_url: t.track.album.images.first().and_then(|i| i.url.clone()),
        },
        spotify_url: t.track.external_urls.spotify.clone(),
        embedding: t.embedding.clone(),
        metadata: Some(metadata),
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

    let response = if params.include_features.unwrap_or(false) {
        let result = spotify
            .search_tracks_with_features(&params.q, params.limit, params.offset)
            .await
            .map_err(|e| AppError::Spotify(e))?;

        SearchResponse {
            tracks: result.tracks.iter().map(track_with_features_to_response).collect(),
            total: result.total,
            limit: result.limit,
            offset: result.offset,
        }
    } else {
        let result = spotify
            .search_tracks(&params.q, params.limit, params.offset)
            .await
            .map_err(|e| AppError::Spotify(e))?;

        SearchResponse {
            tracks: result.tracks.iter().map(track_to_response).collect(),
            total: result.total,
            limit: result.limit,
            offset: result.offset,
        }
    };

    Ok((StatusCode::OK, Json(response)))
}

/// GET /api/v1/tracks/with-features - Fetch tracks by IDs with metadata + embeddings (for Go saga).
pub async fn tracks_with_features(
    State(spotify): State<SpotifyClient>,
    Query(params): Query<TracksWithFeaturesQuery>,
) -> Result<impl IntoResponse, AppError> {
    if params.ids.trim().is_empty() {
        return Err(AppError::BadRequest("ids is required (comma-separated track IDs)".into()));
    }

    let ids: Vec<String> = params.ids.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    if ids.is_empty() {
        return Err(AppError::BadRequest("at least one track id required".into()));
    }

    let tracks = spotify
        .get_tracks_with_features(&ids)
        .await
        .map_err(|e| AppError::Spotify(e))?;

    let response = SearchResponse {
        tracks: tracks.iter().map(track_with_features_to_response).collect(),
        total: tracks.len() as u32,
        limit: tracks.len() as u32,
        offset: 0,
    };

    Ok((StatusCode::OK, Json(response)))
}

/// Build the API router.
pub fn router() -> Router<SpotifyClient> {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/search", get(search))
        .route("/api/v1/tracks/with-features", get(tracks_with_features))
}
