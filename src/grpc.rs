//! gRPC server for Spotify search service.

use tonic::{Request, Response, Status};

use crate::spotify::SpotifyClient;

// Include generated proto code
pub mod spotify_proto {
    tonic::include_proto!("spotify");
}

use spotify_proto::spotify_search_server::{SpotifySearch, SpotifySearchServer};
use spotify_proto::{GetTracksWithFeaturesRequest, GetTracksWithFeaturesResponse, TrackWithFeatures};

/// gRPC service implementation.
pub struct SpotifySearchService {
    spotify: SpotifyClient,
}

impl SpotifySearchService {
    pub fn new(spotify: SpotifyClient) -> Self {
        Self { spotify }
    }

    pub fn into_router(self) -> SpotifySearchServer<SpotifySearchService> {
        SpotifySearchServer::new(self)
    }
}

#[tonic::async_trait]
impl SpotifySearch for SpotifySearchService {
    async fn get_tracks_with_features(
        &self,
        request: Request<GetTracksWithFeaturesRequest>,
    ) -> Result<Response<GetTracksWithFeaturesResponse>, Status> {
        let ids = request.into_inner().track_ids;
        if ids.is_empty() {
            return Ok(Response::new(GetTracksWithFeaturesResponse { tracks: vec![] }));
        }

        let tracks: Vec<TrackWithFeatures> = self
            .spotify
            .get_tracks_with_features(&ids)
            .await
            .map_err(|e| Status::internal(e))?
            .into_iter()
            .filter_map(|t| {
                t.embedding.as_ref().map(|emb| {
                    let mut metadata = std::collections::HashMap::new();
                    metadata.insert("spotify_id".into(), t.track.id.clone());
                    metadata.insert("title".into(), t.track.name.clone());
                    metadata.insert(
                        "artist".into(),
                        t.track.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", "),
                    );
                    metadata.insert("album".into(), t.track.album.name.clone());
                    if let Some(ref url) = t.track.external_urls.spotify {
                        metadata.insert("spotify_url".into(), url.clone());
                    }
                    TrackWithFeatures {
                        id: t.track.id.clone(),
                        embedding: emb.clone(),
                        metadata,
                    }
                })
            })
            .collect();

        Ok(Response::new(GetTracksWithFeaturesResponse { tracks }))
    }
}
