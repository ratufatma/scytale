#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_NODE="${ROOT_DIR}/target/debug/scytale-node"
BIN_CLI="${ROOT_DIR}/target/debug/scytale-cli"
BIN_P2P="${ROOT_DIR}/target/debug/scytale-p2p"

DIR_NODE1="/tmp/scytale_node1_data"
DIR_NODE2="/tmp/scytale_node2_data"
SOCK_NODE1="/tmp/scytale_node1.sock"
SOCK_NODE2="/tmp/scytale_node2.sock"

PORT_P2P_1="9001"
PORT_P2P_2="9002"

LOCK_MINER="010203"

cleanup() {
    echo ""
    echo "[!] Menghentikan seluruh proses testnet..."
    kill "${PID_NODE1:-}" "${PID_NODE2:-}" 2>/dev/null || true
    wait "${PID_NODE1:-}" "${PID_NODE2:-}" 2>/dev/null || true
    rm -rf "${DIR_NODE1}" "${DIR_NODE2}" "${SOCK_NODE1}" "${SOCK_NODE2}"
    echo "[✓] Lingkungan testnet dibersihkan."
}
trap cleanup EXIT INT TERM

echo "============================================================"
echo "          SCYTALE 2-NODE LIVE LOCAL TESTNET HARNESS         "
echo "============================================================"

# 1. Kompilasi Binaries
echo "[1/6] Memeriksa & mengompilasi binary (Rust + Go)..."
cargo build -p scytale-node -p scytale-cli
(cd "${ROOT_DIR}/network" && go build -o "${BIN_P2P}" ./cmd/scytale-p2p)

# Bersihkan sisa data lama
rm -rf "${DIR_NODE1}" "${DIR_NODE2}" "${SOCK_NODE1}" "${SOCK_NODE2}"
mkdir -p "${DIR_NODE1}" "${DIR_NODE2}"

# 2. Jalankan Node 1 (Bootstrap / Miner Node)
echo "[2/6] Menjalankan Node 1 (Port P2P: ${PORT_P2P_1}, IPC: ${SOCK_NODE1})..."
"${BIN_NODE}" \
    --data-dir "${DIR_NODE1}" \
    --socket "${SOCK_NODE1}" \
    start \
    --p2p-bind "127.0.0.1:${PORT_P2P_1}" \
    --p2p-bin "${BIN_P2P}" \
    --target "0x207fffff" \
    > "/tmp/scytale_node1.log" 2>&1 &
PID_NODE1=$!

# Tunggu IPC socket Node 1 aktif
for i in {1..30}; do
    if [ -S "${SOCK_NODE1}" ]; then break; fi
    sleep 0.2
done
if [ ! -S "${SOCK_NODE1}" ]; then
    echo "[-] Gagal memulai Node 1. Log: /tmp/scytale_node1.log"
    cat /tmp/scytale_node1.log
    exit 1
fi
echo "     [+] Node 1 aktif (PID: ${PID_NODE1})."

# 3. Jalankan Node 2 (Peer / Follower Node)
echo "[3/6] Menjalankan Node 2 (Port P2P: ${PORT_P2P_2}, Peer: 127.0.0.1:${PORT_P2P_1})..."
"${BIN_NODE}" \
    --data-dir "${DIR_NODE2}" \
    --socket "${SOCK_NODE2}" \
    start \
    --p2p-bind "127.0.0.1:${PORT_P2P_2}" \
    --peer "127.0.0.1:${PORT_P2P_1}" \
    --p2p-bin "${BIN_P2P}" \
    --target "0x207fffff" \
    > "/tmp/scytale_node2.log" 2>&1 &
PID_NODE2=$!

# Tunggu IPC socket Node 2 aktif
for i in {1..30}; do
    if [ -S "${SOCK_NODE2}" ]; then break; fi
    sleep 0.2
done
if [ ! -S "${SOCK_NODE2}" ]; then
    echo "[-] Gagal memulai Node 2. Log: /tmp/scytale_node2.log"
    cat /tmp/scytale_node2.log
    exit 1
fi
echo "     [+] Node 2 aktif (PID: ${PID_NODE2})."

# 4. Status Awal Kedua Node
echo "[4/6] Memeriksa status awal kedua node..."
echo "--- Status Node 1 ---"
"${BIN_CLI}" --socket "${SOCK_NODE1}" status
echo "--- Status Node 2 ---"
"${BIN_CLI}" --socket "${SOCK_NODE2}" status

# 5. Menambang Blok di Node 1
echo "[5/6] Mengaktifkan penambangan pada Node 1..."
"${BIN_CLI}" --socket "${SOCK_NODE1}" mine --start

echo "     [*] Menambang blok di Node 1 selama 1 detik..."
sleep 1

"${BIN_CLI}" --socket "${SOCK_NODE1}" mine --stop
echo "     [+] Penambangan dihentikan."

# Tunggu sejenak agar worker mining berhenti sepenuhnya dan propagasi tuntas
sleep 0.5

# 6. Verifikasi Sinkronisasi Rantai Antar-Proses
echo "[6/6] Verifikasi hasil propagasi rantai P2P..."

STATUS_1=$("${BIN_CLI}" --socket "${SOCK_NODE1}" status)
HEIGHT_1=$(echo "${STATUS_1}" | grep -i "Canonical Height" | awk '{print $NF}')
TIP_1=$(echo "${STATUS_1}" | grep -i "Canonical Tip" | awk '{print $NF}')

# Tunggu proses propagasi dan sinkronisasi IBD
for i in {1..50}; do
    STATUS_2=$("${BIN_CLI}" --socket "${SOCK_NODE2}" status)
    HEIGHT_2=$(echo "${STATUS_2}" | grep -i "Canonical Height" | awk '{print $NF}')
    TIP_2=$(echo "${STATUS_2}" | grep -i "Canonical Tip" | awk '{print $NF}')
    if [ "${HEIGHT_2}" -eq "${HEIGHT_1}" ] && [ "${TIP_2}" == "${TIP_1}" ]; then
        break
    fi
    sleep 0.1
done

echo "============================================================"
echo "HASIL KONSENSUS DUA NODE:"
echo "Node 1 Height: ${HEIGHT_1} | Tip: ${TIP_1}"
echo "Node 2 Height: ${HEIGHT_2} | Tip: ${TIP_2}"
echo "============================================================"

if [ "${HEIGHT_1}" -gt 0 ] && [ "${HEIGHT_1}" -eq "${HEIGHT_2}" ] && [ "${TIP_1}" == "${TIP_2}" ]; then
    echo "[✓] SUKSES: Node 2 tersinkronisasi 100% dengan rantai kanonikal Node 1!"
else
    echo "[x] PERINGATAN: Tinggi blok atau hash tip belum identik. Berikan waktu sinkronisasi..."
    sleep 2
    STATUS_2=$("${BIN_CLI}" --socket "${SOCK_NODE2}" status)
    echo "${STATUS_2}"
fi

echo ""
echo "--- Buku Tabungan (Passbook) Hasil Tambang di Node 2 ---"
"${BIN_CLI}" --socket "${SOCK_NODE2}" passbook --lock "${LOCK_MINER}"
echo "============================================================"