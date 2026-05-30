# Multi-stage build for the Rust ingestion pipeline.
# rustls everywhere (sqlx + reqwest), so no OpenSSL/libssl needed at build time.
FROM rust:1.95-slim-bookworm AS build
WORKDIR /build
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /build/target/release/gear-nest-pipeline /usr/local/bin/gear-nest-pipeline
# The migration runner walks up from the cwd looking for supabase/migrations,
# which docker-compose mounts at /app/supabase/migrations.
ENTRYPOINT ["gear-nest-pipeline"]
