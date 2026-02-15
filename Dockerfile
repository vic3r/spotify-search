# Build stage. Build from monorepo root: docker build -f spotify-search/Dockerfile .
FROM rust:1.83-bookworm AS builder
WORKDIR /app

COPY spotify-search/Cargo.toml spotify-search/build.rs ./
COPY spotify-search/src ./src
COPY proto ./proto
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app

COPY --from=builder /app/target/release/spotify-search .
EXPOSE 8081 50051

ENV PORT=8081
ENV GRPC_PORT=50051
ENTRYPOINT ["./spotify-search"]
