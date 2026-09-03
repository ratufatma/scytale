# --- STAGE 1: Build Rust Binaries (Full Containerized Pipeline) ---
FROM rust:1.80-slim-bookworm AS rust-builder
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY apps apps
COPY web web
RUN cargo build --release -p scytale-node -p scytale-cli

# --- STAGE 2: Build Go P2P Daemon (Full Containerized Pipeline) ---
FROM golang:1.22-bookworm AS go-builder
WORKDIR /app/network
COPY network/go.mod network/go.sum* ./
RUN go mod download
COPY network/ ./
RUN CGO_ENABLED=0 go build -ldflags="-s -w" -o /app/bin/scytale-p2p ./cmd/scytale-p2p
RUN CGO_ENABLED=0 go build -ldflags="-s -w" -o /app/bin/scytale-seeder ./cmd/scytale-seeder

# --- STAGE 3: Minimal Production Runtime Image ---
FROM ubuntu:24.04
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    jq \
    iptables \
    iproute2 \
    procps \
    dnsutils \
    tini && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY target/release/scytale-node /usr/local/bin/scytale-node
COPY target/release/scytale-cli /usr/local/bin/scytale-cli
COPY target/release/scytale-p2p /usr/local/bin/scytale-p2p
COPY target/release/scytale-seeder /usr/local/bin/scytale-seeder

# Default runtime and state directories
RUN mkdir -p /data /run/scytale /root/.scytale

ENV RUST_LOG=info
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["scytale-node", "--data-dir", "/data", "--socket", "/run/scytale/node.sock", "start"]
