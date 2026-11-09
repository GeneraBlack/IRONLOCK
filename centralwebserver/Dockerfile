# Multi-Stage Containerfile / Dockerfile für NethServer 8 (Podman) & Docker
FROM docker.io/library/node:22.14.0-alpine3.21 AS builder

WORKDIR /app
COPY package.json package-lock.json* ./
RUN npm ci

COPY tsconfig.json tsconfig.client.json ./
COPY src ./src
COPY test ./test
COPY public ./public

RUN npm run build

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
ENV RELAY_PORT_START=30100
ENV RELAY_PORT_END=30139

LABEL org.opencontainers.image.title="IRONLOCK // RUST-RTS Server" \
      org.opencontainers.image.version="${MODULE_VERSION}"

COPY --from=builder /app/package.json ./package.json
COPY --from=builder /app/dist ./dist
COPY --from=builder /app/public ./public

EXPOSE 8080
EXPOSE 30100-30139

CMD ["node", "dist/src/index.js"]
