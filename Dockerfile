# sAIles: one image with the Rust server and the built dashboard.

FROM node:22-bookworm-slim AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:1-bookworm AS server
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p sailes-server

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 sailes
WORKDIR /app
COPY --from=server /src/target/release/sailes /usr/local/bin/sailes
COPY --from=web /web/dist /app/web
ENV SAILES_WEB_DIR=/app/web \
    SAILES_BIND=0.0.0.0:8080 \
    RUST_LOG=sailes=info,tower_http=info
USER sailes
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=20s CMD curl -fsS http://127.0.0.1:8080/api/health || exit 1
CMD ["sailes", "serve"]
