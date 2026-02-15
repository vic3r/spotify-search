mod config;
mod error;
mod grpc;
mod handlers;
mod spotify;

use std::net::SocketAddr;

use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Config;
use crate::grpc::SpotifySearchService;
use crate::handlers::router;
use crate::spotify::SpotifyClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;
    let spotify = SpotifyClient::new(config.spotify_client_id.clone(), config.spotify_client_secret.clone());

    let grpc_svc = SpotifySearchService::new(spotify.clone());
    let grpc_router = grpc_svc.into_router();

    let app = router()
        .layer(TraceLayer::new_for_http())
        .with_state(spotify);

    let http_addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let grpc_addr = SocketAddr::from(([0, 0, 0, 0], config.grpc_port));

    tracing::info!("HTTP listening on {}", http_addr);
    tracing::info!("gRPC listening on {}", grpc_addr);

    let grpc_server = tonic::transport::Server::builder()
        .add_service(grpc_router)
        .serve(grpc_addr);

    tokio::select! {
        r = axum::serve(
            tokio::net::TcpListener::bind(http_addr).await?,
            app.into_make_service(),
        ) => r?,
        r = grpc_server => r?,
    }

    Ok(())
}
