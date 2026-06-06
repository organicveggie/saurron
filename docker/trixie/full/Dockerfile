# Stage 1: Frontend build
FROM node:22-bookworm-slim AS node-builder

WORKDIR /build/web
COPY web/package.json web/pnpm-lock.yaml web/pnpm-workspace.yaml ./
RUN npm install -g pnpm && pnpm install --frozen-lockfile
COPY web/ ./
RUN pnpm build

# Stage 2: Rust build
FROM rust:1-trixie AS builder

ARG SAURRON_BUILD_VERSION=v0.0.0-unknown
ENV SAURRON_BUILD_VERSION=${SAURRON_BUILD_VERSION}

WORKDIR /build
COPY . .
COPY --from=node-builder /build/web/dist ./web/dist
RUN cargo build --profile release --locked --features web

# Stage 3: Runtime
FROM debian:trixie AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*

# Non-root user. GID 0 lets the container access a Docker socket mounted
# with group-write permissions (a common operator pattern).
RUN useradd -m -u 1000 -g 0 saurron

COPY --from=builder /build/target/release/saurron /usr/local/bin/saurron

ARG SAURRON_BUILD_VERSION=v0.0.0-unknown

LABEL org.opencontainers.image.title="Saurron" \
      org.opencontainers.image.description="Ever-watchful eye for your Docker containers. Automatic container updater with rollback, audit trail, notifications, and Prometheus metrics." \
      org.opencontainers.image.version="${SAURRON_BUILD_VERSION}" \
      org.opencontainers.image.licenses="GPL-3.0-or-later" \
      org.opencontainers.image.source="https://github.com/organicveggie/saurron"

ENV TZ=UTC

USER saurron

# HTTP API port (only active when --http-api-update or --http-api-metrics is set)
EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/saurron"]
