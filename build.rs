fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let local = manifest.join("proto/spotify.proto");
    let monorepo = manifest.join("../../proto/spotify.proto");
    let (path, inc) = if local.exists() {
        (local, manifest.join("proto"))
    } else if monorepo.exists() {
        (manifest.join("../../proto/spotify.proto").canonicalize()?, manifest.join("../../proto").canonicalize()?)
    } else {
        return Err("proto/spotify.proto not found (run from monorepo root or copy proto into spotify-search)".into());
    };
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile(&[path], &[inc])?;
    Ok(())
}
