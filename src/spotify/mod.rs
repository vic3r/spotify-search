//! Spotify Web API client.
//!
//! Uses Client Credentials flow for server-to-server authentication.

use reqwest::Client;
use serde::Deserialize;

const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const API_BASE: &str = "https://api.spotify.com/v1";

/// Spotify API client with token caching.
#[derive(Clone)]
pub struct SpotifyClient {
    client: Client,
    client_id: String,
    client_secret: String,
    token: tokio::sync::RwLock<Option<CachedToken>>,
}

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
            token: tokio::sync::RwLock::new(None),
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
