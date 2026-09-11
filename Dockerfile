# --- STAGE 1: Build Rust Binaries ---
FROM rust:latest AS rust-builder
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY apps apps
COPY contracts contracts
COPY explorer explorer
RUN cargo build --release -p scytale-node -p scytale-cli

# --- STAGE 2: Minimal Production Runtime Image ---
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
COPY --from=rust-builder /app/target/release/scytale-node /usr/local/bin/scytale-node
COPY --from=rust-builder /app/target/release/scytale-cli /usr/local/bin/scytale-cli

# Default runtime and state directories
RUN mkdir -p /data /run/scytale /root/.scytale

ENV RUST_LOG=info
ENV SCYTALE_SOCKET=/run/scytale/node.sock
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["scytale-node", "--data-dir", "/data", "--socket", "/run/scytale/node.sock", "start"]
