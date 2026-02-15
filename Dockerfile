# Build stage
FROM rust:1.83-bookworm AS builder
WORKDIR /app

COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app

COPY --from=builder /app/target/release/spotify-search .
EXPOSE 8081

ENV PORT=8081
ENTRYPOINT ["./spotify-search"]
