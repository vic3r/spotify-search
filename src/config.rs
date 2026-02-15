use std::env;

/// Application configuration from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub grpc_port: u16,
    pub spotify_client_id: String,
    pub spotify_client_secret: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8081);

        let grpc_port = env::var("GRPC_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(50051);

        let spotify_client_id = env::var("SPOTIFY_CLIENT_ID")
            .map_err(|_| anyhow::anyhow!("SPOTIFY_CLIENT_ID is required"))?;

        let spotify_client_secret = env::var("SPOTIFY_CLIENT_SECRET")
            .map_err(|_| anyhow::anyhow!("SPOTIFY_CLIENT_SECRET is required"))?;

        Ok(Self {
            port,
            grpc_port,
            spotify_client_id,
            spotify_client_secret,
        })
    }
}
