# syntax=docker/dockerfile:1

# --- Frontend bauen ---------------------------------------------------------
FROM node:24-alpine AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# --- Rust: Abhängigkeiten vorab bauen (Cache über cargo-chef) ---------------
FROM lukemathwalker/cargo-chef:latest-rust-alpine AS chef
RUN apk add --no-cache musl-dev
WORKDIR /app

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --locked --recipe-path recipe.json
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY --from=web /web/dist ./web/dist
RUN cargo build --release --locked

# --- Laufzeit-Image ---------------------------------------------------------
FROM alpine:3.22

RUN apk add --no-cache su-exec tzdata

COPY --from=builder /app/target/release/sebo /usr/local/bin/sebo
COPY --chmod=755 docker/entrypoint.sh /entrypoint.sh
COPY --chmod=755 docker/sebo-as-user /usr/local/bin/sebo-as-user
RUN ln -s sebo-as-user /usr/local/bin/reset-2fa \
 && ln -s sebo-as-user /usr/local/bin/hash-password

ENV PUID=99 \
    PGID=100 \
    TZ=Europe/Berlin \
    PORT=8080 \
    DATA_DIR=/data

VOLUME /data
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD wget -q -O /dev/null "http://127.0.0.1:${PORT}/healthz" || exit 1

LABEL org.opencontainers.image.title="SEBO" \
      org.opencontainers.image.description="Self-hosted Bookmarks" \
      org.opencontainers.image.source="https://github.com/mr-glubschi/sebo"

ENTRYPOINT ["/entrypoint.sh"]
CMD ["serve"]
