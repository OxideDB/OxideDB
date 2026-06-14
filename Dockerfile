# syntax=docker/dockerfile:1.7

ARG NODE_VERSION=20
ARG RUST_VERSION=1
ARG APP_UID=10001
ARG APP_GID=10001

FROM node:${NODE_VERSION}-bookworm-slim AS ui-builder
WORKDIR /app/ui

COPY ui/package.json ui/package-lock.json ./
RUN npm ci

COPY ui/ ./
RUN npm run build

FROM rust:${RUST_VERSION}-bookworm AS rust-builder
WORKDIR /app

COPY . .
COPY --from=ui-builder /app/ui/dist ./ui/dist

# Keep the embedded UI artifact newer than source files so oxide-api/build.rs
# does not try to invoke npm inside the Rust-only build stage.
RUN touch ui/dist && cargo build --locked --release --bin oxidedb

FROM debian:bookworm-slim AS runtime

ARG APP_UID
ARG APP_GID

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl libgcc-s1 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid "${APP_GID}" oxidedb \
    && useradd --system --uid "${APP_UID}" --gid "${APP_GID}" --home-dir /app --shell /usr/sbin/nologin oxidedb \
    && mkdir -p /app /data /plugins /logs \
    && chown -R oxidedb:oxidedb /app /data /plugins /logs

COPY --from=rust-builder /app/target/release/oxidedb /usr/local/bin/oxidedb

WORKDIR /app

ENV OXIDEDB_ENV=production \
    OXIDEDB_COOKIE_SECURE=true \
    OXIDEDB_COOKIE_SAME_SITE=Lax \
    RUST_LOG=info

EXPOSE 8080
VOLUME ["/data", "/plugins", "/logs"]

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -fsS -H "x-forwarded-proto: https" http://127.0.0.1:8080/health || exit 1

USER oxidedb

ENTRYPOINT ["oxidedb"]
CMD ["start", "--bind-address", "0.0.0.0", "--api-port", "8080", "--db-path", "/data/oxidedb.sqlite", "--plugin-folder", "/plugins", "--logging-db-path", "/logs", "--security-policy", "strict"]
