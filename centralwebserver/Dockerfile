# Multi-Stage Containerfile / Dockerfile für NethServer 8 (Podman) & Docker
# Build from repository root: podman build -f centralwebserver/Dockerfile .

# Stage 1: Build Headless Authoritative Dedicated Server (rts-dedicated-server)
FROM docker.io/library/rust:1.85.0-alpine3.21 AS dedicated-builder

RUN apk add --no-cache musl-dev pkgconfig

WORKDIR /workspace/engine
COPY engine/Cargo.toml engine/Cargo.lock* ./
COPY engine/crates ./crates

RUN cargo build --release -p rts-network --bin rts-dedicated-server && \
    strip /workspace/engine/target/release/rts-dedicated-server

# Stage 2: Build Central Webserver & Frontend Portal
FROM docker.io/library/node:22.14.0-alpine3.21 AS builder

WORKDIR /app
COPY centralwebserver/package.json centralwebserver/package-lock.json* ./
RUN npm install --no-audit --no-fund

COPY centralwebserver/tsconfig.json centralwebserver/tsconfig.client.json ./
COPY centralwebserver/src ./src
COPY centralwebserver/test ./test
COPY centralwebserver/public ./public

RUN npm run build

# Stage 3: Production Runtime Bundling Central Webserver + rts-dedicated-server
FROM docker.io/library/node:22.14.0-alpine3.21 AS runtime

ARG MODULE_VERSION=1.0.0

WORKDIR /app
ENV NODE_ENV=production
ENV HOST=0.0.0.0
ENV PORT=8080
ENV PUBLIC_DOMAIN=rust-rts.bls-isp.net
ENV PUBLIC_HOST=rust-rts.bls-isp.net
ENV NODE_ID=ns8-bls-isp-01
ENV TTL_SECS=45
ENV DATA_DIR=/app/data
ENV RELAY_PORT_START=30100
ENV RELAY_PORT_END=30139
ENV DEDICATED_SERVER_BIN=/usr/local/bin/rts-dedicated-server

LABEL org.opencontainers.image.title="IRONLOCK // RUST-RTS Server" \
      org.opencontainers.image.version="${MODULE_VERSION}"

COPY --from=dedicated-builder /workspace/engine/target/release/rts-dedicated-server /usr/local/bin/rts-dedicated-server
COPY --from=builder --chown=node:node /app/package.json ./package.json
COPY --from=builder --chown=node:node /app/dist ./dist
COPY --from=builder --chown=node:node /app/public ./public

RUN chmod 0755 /usr/local/bin/rts-dedicated-server && \
    mkdir -p /app/data && chown -R node:node /app/data

USER node

EXPOSE 8080
EXPOSE 30100-30139

CMD ["node", "dist/src/index.js"]
