#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

PROJECT_NAME="scytale_seeder_test"

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

info() { echo -e "${BLUE}[INFO]${NC} $*"; }
success() { echo -e "${GREEN}[✓]${NC} $*"; }
warn() { echo -e "${YELLOW}[!]${NC} $*"; }
fail() { echo -e "${RED}[✗]${NC} $*"; exit 1; }

cleanup() {
    warn "Cleaning up Docker test resources..."
    docker compose -p "$PROJECT_NAME" --profile coldstart down -v --remove-orphans >/dev/null 2>&1 || true
    info "Cleanup complete."
}
trap cleanup EXIT INT TERM

wait_for_http() {
    local port=$1
    local name=$2
    local max_attempts=30
    local attempt=0

    info "Waiting for $name (HTTP port $port) to be ready..."
    while [ $attempt -lt $max_attempts ]; do
        if curl -s -f "http://127.0.0.1:$port/api/v1/status" >/dev/null 2>&1; then
            success "$name is responsive on port $port."
            return 0
        fi
        attempt=$((attempt + 1))
        sleep 1
    done
    fail "Timeout waiting for $name on port $port."
}

echo -e "${CYAN}============================================================${NC}"
echo -e "${CYAN}   SCYTALE DOCKER SEEDER & COLD-START BOOTSTRAP TEST       ${NC}"
echo -e "${CYAN}============================================================${NC}"

info "[1/4] Compiling binaries and building container image..."
if [ ! -f "target/release/scytale-node" ] || [ ! -f "target/release/scytale-cli" ]; then
    info "Compiling Rust release binaries..."
    cargo build --release -p scytale-node -p scytale-cli
fi

if [ ! -f "target/release/scytale-p2p" ]; then
    info "Compiling Go P2P binary..."
    (cd network && CGO_ENABLED=0 go build -ldflags="-s -w" -o ../target/release/scytale-p2p ./cmd/scytale-p2p)
fi

if [ ! -f "target/release/scytale-seeder" ]; then
    info "Compiling Go DNS Seeder binary..."
    (cd network && CGO_ENABLED=0 go build -ldflags="-s -w" -o ../target/release/scytale-seeder ./cmd/scytale-seeder)
fi

docker compose -p "$PROJECT_NAME" --profile coldstart down -v --remove-orphans >/dev/null 2>&1 || true
docker compose -p "$PROJECT_NAME" build

info "[2/4] Starting Seeder, node-1 (Miner), and node-2 (Relay)..."
docker compose -p "$PROJECT_NAME" --profile coldstart up -d seeder node-1 node-2

wait_for_http 8332 "node-1"
wait_for_http 8333 "node-2"

info "Waiting for node-1 to mine at least 10 blocks..."
attempt=0
while [ $attempt -lt 40 ]; do
    HEIGHT=$(curl -s "http://127.0.0.1:8332/api/v1/status" | jq -r '.canonical_height // 0')
    if [ "$HEIGHT" -ge 10 ]; then
        success "node-1 mined $HEIGHT blocks."
        break
    fi
    attempt=$((attempt + 1))
    sleep 1
done

if [ "$HEIGHT" -lt 10 ]; then
    fail "node-1 failed to mine 10 blocks (current: $HEIGHT)."
fi

info "Pausing mining on node-1 to freeze state for cold-start sync verification..."
docker exec scytale-node1 scytale-cli --socket /run/scytale/node.sock mine stop >/dev/null 2>&1 || true
docker exec scytale-node1 scytale-cli --socket /run/scytale/node.sock mine --stop >/dev/null 2>&1 || true
sleep 2

info "[3/4] Verifying DNS Seeder resolves healthy nodes..."
sleep 5
SEEDER_RESPONSE=$(dig @127.0.0.1 -p 1053 seed.scytale.org A +short || true)
info "DNS Seeder response for seed.scytale.org: $SEEDER_RESPONSE"

if [ -z "$SEEDER_RESPONSE" ]; then
    warn "Direct host dig returned empty; checking inside cluster network..."
    SEEDER_RESPONSE=$(docker exec scytale-seeder dig @127.0.0.1 seed.scytale.org A +short || true)
    info "Cluster internal dig response: $SEEDER_RESPONSE"
fi

info "[4/4] Starting node-coldstart with ZERO static peers..."
docker compose -p "$PROJECT_NAME" --profile coldstart up -d node-coldstart
wait_for_http 8336 "node-coldstart"

info "Waiting for node-coldstart to discover peers via DNS and sync blocks..."
synced=false
for i in $(seq 1 45); do
    CS_HEIGHT=$(curl -s "http://127.0.0.1:8336/api/v1/status" | jq -r '.canonical_height // 0')
    N1_HEIGHT=$(curl -s "http://127.0.0.1:8332/api/v1/status" | jq -r '.canonical_height // 0')
    info "Progress [${i}s]: node-coldstart height = $CS_HEIGHT, node-1 height = $N1_HEIGHT"

    if [ "$CS_HEIGHT" -ge 5 ] && [ "$CS_HEIGHT" -ge "$((N1_HEIGHT - 1))" ]; then
        synced=true
        success "node-coldstart successfully synced to height $CS_HEIGHT!"
        break
    fi
    sleep 1
done

if [ "$synced" = false ]; then
    warn "node-coldstart logs:"
    docker compose -p "$PROJECT_NAME" logs node-coldstart || true
    fail "node-coldstart failed to sync via DNS seed within timeout."
fi

CS_ROOT=$(curl -s "http://127.0.0.1:8336/api/v1/status" | jq -r '.utxo_root // empty')
N1_ROOT=$(curl -s "http://127.0.0.1:8332/api/v1/status" | jq -r '.utxo_root // empty')

if [ "$CS_ROOT" = "$N1_ROOT" ] && [ -n "$CS_ROOT" ]; then
    success "UTXO commitment verified: utxo_root matches perfectly ($CS_ROOT)."
else
    fail "UTXO commitment mismatch: node-coldstart ($CS_ROOT) vs node-1 ($N1_ROOT)."
fi

echo -e "${GREEN}============================================================${NC}"
echo -e "${GREEN}   AUTONOMOUS DNS SEEDER & COLD-START BOOTSTRAP PASSED!     ${NC}"
echo -e "${GREEN}============================================================${NC}"
