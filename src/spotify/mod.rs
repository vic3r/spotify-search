//! Spotify Web API client.
//!
//! Uses Client Credentials flow for server-to-server authentication.

use std::sync::Arc;

use base64::Engine;
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::RwLock;

const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const API_BASE: &str = "https://api.spotify.com/v1";

/// Spotify API client with token caching.
#[derive(Clone)]
pub struct SpotifyClient {
    client: Client,
    client_id: String,
    client_secret: String,
    token: Arc<RwLock<Option<CachedToken>>>,
}

#[derive(Clone)]
struct CachedToken {
    access_token: String,
    expires_at: std::time::Instant,
}

impl SpotifyClient {
    pub fn new(client_id: String, client_secret: String) -> Self {
        Self {
            client: Client::new(),
            client_id,
            client_secret,
            token: Arc::new(RwLock::new(None)),
        }
    }

    /// Ensures we have a valid access token, refreshing if needed.
    async fn ensure_token(&self) -> Result<String, String> {
        {
            let guard = self.token.read().await;
            if let Some(ref t) = *guard {
                if t.expires_at > std::time::Instant::now() {
                    return Ok(t.access_token.clone());
                }
            }
        }

        let token = self.fetch_token().await?;
        {
            let mut guard = self.token.write().await;
            *guard = Some(token.clone());
        }
        Ok(token.access_token)
    }

    async fn fetch_token(&self) -> Result<CachedToken, String> {
        let params = [
            ("grant_type", "client_credentials"),
        ];
        let auth = base64::engine::general_purpose::STANDARD.encode(
            format!("{}:{}", self.client_id, self.client_secret).as_bytes(),
        );

        let res = self
            .client
            .post(TOKEN_URL)
            .header("Authorization", format!("Basic {}", auth))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("token request failed: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("token request failed: {} - {}", status, body));
        }

        let body: TokenResponse = res.json().await.map_err(|e| format!("token parse failed: {}", e))?;
        let expires_at = std::time::Instant::now() + std::time::Duration::from_secs(body.expires_in.saturating_sub(60));

        Ok(CachedToken {
            access_token: body.access_token,
            expires_at,
        })
    }

    /// Search for tracks in the Spotify catalog.
    pub async fn search_tracks(&self, q: &str, limit: Option<u32>, offset: Option<u32>) -> Result<SearchTracksResponse, String> {
        let token = self.ensure_token().await?;

        let limit = limit.unwrap_or(20).min(50).max(1);
        let offset = offset.unwrap_or(0).min(1000);

        let url = format!("{}/search?q={}&type=track&limit={}&offset={}",
            API_BASE,
            urlencoding::encode(q),
            limit,
            offset,
        );

        let res = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| format!("search request failed: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("Spotify API error {}: {}", status, body));
        }

        let body: SearchResponse = res.json().await.map_err(|e| format!("search parse failed: {}", e))?;
        Ok(SearchTracksResponse {
            tracks: body.tracks.items,
            total: body.tracks.total,
            limit: body.tracks.limit,
            offset: body.tracks.offset,
        })
    }

    /// Fetch track metadata for up to 50 IDs. Returns Some for each id, or None if not available.
    pub async fn get_tracks(&self, ids: &[String]) -> Result<Vec<Option<Track>>, String> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let ids: Vec<_> = ids.iter().take(50).cloned().collect();
        let ids_param = ids.join(","");

        let token = self.ensure_token().await?;
        let url = format!("{}/tracks?ids={}", API_BASE, urlencoding::encode(&ids_param));

        let res = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| format!("tracks request failed: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("Spotify API error {}: {}", status, body));
        }

        let body: TracksResponse = res.json().await.map_err(|e| format!("tracks parse failed: {}", e))?;
        Ok(body.tracks)
    }

    /// Fetch track metadata + audio features for given IDs. For Go saga: merge and return with embeddings.
    pub async fn get_tracks_with_features(&self, ids: &[String]) -> Result<Vec<TrackWithFeatures>, String> {
        let ids: Vec<_> = ids.iter().take(50).cloned().collect();
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let (tracks_result, features_result) = tokio::join!(
            self.get_tracks(&ids),
            self.get_audio_features(&ids),
        );

        let tracks = tracks_result?;
        let features = features_result?;

        let mut result = Vec::with_capacity(ids.len());
        for i in 0..ids.len() {
            let track = tracks.get(i).and_then(|t| t.clone());
            let audio_features = features.get(i).and_then(|f| f.clone());
            let embedding = audio_features.as_ref().map(|af| af.to_embedding());

            if let Some(track) = track {
                result.push(TrackWithFeatures {
                    track,
                    audio_features,
                    embedding,
                });
            }
        }
        Ok(result)
    }

    /// Fetch audio features for up to 100 track IDs. Returns Some for each id, or None if not available.
    pub async fn get_audio_features(&self, ids: &[String]) -> Result<Vec<Option<AudioFeatures>>, String> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let ids: Vec<_> = ids.iter().take(100).cloned().collect();
        let ids_param = ids.join(","");

        let token = self.ensure_token().await?;
        let url = format!("{}/audio-features?ids={}", API_BASE, urlencoding::encode(&ids_param));

        let res = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| format!("audio-features request failed: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("Spotify API error {}: {}", status, body));
        }

        let body: AudioFeaturesResponse = res.json().await.map_err(|e| format!("audio-features parse failed: {}", e))?;
        Ok(body.audio_features)
    }

    /// Search tracks and fetch audio features for each. Returns tracks with embeddings.
    pub async fn search_tracks_with_features(
        &self,
        q: &str,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<SearchTracksWithFeaturesResponse, String> {
        let result = self.search_tracks(q, limit, offset).await?;
        let ids: Vec<String> = result.tracks.iter().map(|t| t.id.clone()).collect();

        let features = if ids.is_empty() {
            vec![]
        } else {
            self.get_audio_features(&ids).await?
        };

        let mut tracks_with_features = Vec::with_capacity(result.tracks.len());
        for (i, track) in result.tracks.into_iter().enumerate() {
            let audio_features = features.get(i).and_then(|f| f.clone());
            let embedding = audio_features.as_ref().map(|af| af.to_embedding());
            tracks_with_features.push(TrackWithFeatures {
                track,
                audio_features,
                embedding,
            });
        }

        Ok(SearchTracksWithFeaturesResponse {
            tracks: tracks_with_features,
            total: result.total,
            limit: result.limit,
            offset: result.offset,
        })
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
}

#[derive(Deserialize)]
struct SearchResponse {
    tracks: TracksPage,
}

#[derive(Deserialize)]
struct TracksPage {
    items: Vec<Track>,
    total: u32,
    limit: u32,
    offset: u32,
}

/// Response from track search.
pub struct SearchTracksResponse {
    pub tracks: Vec<Track>,
    pub total: u32,
    pub limit: u32,
    pub offset: u32,
}

/// A Spotify track (simplified).
#[derive(Clone, Debug, Deserialize)]
pub struct Track {
    pub id: String,
    pub name: String,
    pub uri: String,
    #[serde(default)]
    pub duration_ms: u32,
    #[serde(default)]
    pub explicit: bool,
    #[serde(default)]
    pub artists: Vec<Artist>,
    #[serde(default)]
    pub album: Album,
    #[serde(default)]
    pub external_urls: ExternalUrls,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct Artist {
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub external_urls: ExternalUrls,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct Album {
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub images: Vec<Image>,
    #[serde(default)]
    pub external_urls: ExternalUrls,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct Image {
    pub url: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ExternalUrls {
    pub spotify: Option<String>,
}

// ---------------------------------------------------------------------------
// Audio Features (GET /v1/audio-features)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
pub struct AudioFeatures {
    pub id: Option<String>,
    #[serde(default)]
    pub acousticness: f32,
    #[serde(default)]
    pub danceability: f32,
    #[serde(default)]
    pub energy: f32,
    #[serde(default)]
    pub instrumentalness: f32,
    #[serde(default = "default_key")]
    pub key: i32,
    #[serde(default)]
    pub liveness: f32,
    #[serde(default)]
    pub loudness: f32,
    #[serde(default)]
    pub mode: i32,
    #[serde(default)]
    pub speechiness: f32,
    #[serde(default)]
    pub tempo: f32,
    #[serde(default = "default_time_signature")]
    pub time_signature: i32,
    #[serde(default)]
    pub valence: f32,
}

fn default_key() -> i32 { -1 }
fn default_time_signature() -> i32 { 4 }

impl AudioFeatures {
    /// Convert audio features to a 12-dimensional embedding for cosine similarity.
    /// All values normalized to approximately 0-1 range.
    pub fn to_embedding(&self) -> Vec<f32> {
        let key_norm = ((self.key + 1) as f32) / 12.0; // -1..11 -> 0..1
        let loudness_norm = ((self.loudness + 60.0) / 60.0).clamp(0.0, 1.0); // ~-60..0 -> 0..1
        let mode_norm = self.mode as f32; // 0 or 1
        let tempo_norm = (self.tempo / 250.0).clamp(0.0, 1.0); // 0..250 -> 0..1
        let time_sig_norm = ((self.time_signature - 3) as f32) / 4.0; // 3..7 -> 0..1

        vec![
            self.acousticness.clamp(0.0, 1.0),
            self.danceability.clamp(0.0, 1.0),
            self.energy.clamp(0.0, 1.0),
            self.instrumentalness.clamp(0.0, 1.0),
            key_norm,
            self.liveness.clamp(0.0, 1.0),
            loudness_norm,
            mode_norm,
            self.speechiness.clamp(0.0, 1.0),
            tempo_norm,
            time_sig_norm,
            self.valence.clamp(0.0, 1.0),
        ]
    }
}

#[derive(Deserialize)]
struct TracksResponse {
    tracks: Vec<Option<Track>>,
}

#[derive(Deserialize)]
struct AudioFeaturesResponse {
    audio_features: Vec<Option<AudioFeatures>>,
}

/// Track with optional audio features and embedding.
#[derive(Clone, Debug)]
pub struct TrackWithFeatures {
    pub track: Track,
    pub audio_features: Option<AudioFeatures>,
    pub embedding: Option<Vec<f32>>,
}

/// Search response with tracks and audio features/embeddings.
pub struct SearchTracksWithFeaturesResponse {
    pub tracks: Vec<TrackWithFeatures>,
    pub total: u32,
    pub limit: u32,
    pub offset: u32,
}
