#!/usr/bin/env bash
# ==============================================================================
# Scytale Chaos Testnet: Dynamic Peer Connect & Live Fork Reorganization Harness
# ==============================================================================
# Skenario:
# 1. Jalankan Node 1 (Port 9001) dan Node 2 (Port 9002) secara terisolasi.
# 2. Node 1 menambang cabang minoritas (2 blok, Height = 2).
# 3. Node 2 menambang cabang mayoritas (5 blok, Height = 5).
# 4. Verifikasi bahwa kedua node memiliki hash tip yang berbeda (chain split).
# 5. Picu koneksi runtime: `scytale-cli peer connect 127.0.0.1:9002` pada Node 1.
# 6. Tunggu IBD dan buktikan reorganisasi rantai hidup (Node 1 mengadopsi cabang Node 2).
# ==============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_NODE="${ROOT_DIR}/target/debug/scytale-node"
BIN_CLI="${ROOT_DIR}/target/debug/scytale-cli"
BIN_P2P="${ROOT_DIR}/target/debug/scytale-p2p"

DIR_NODE1="/tmp/scytale_reorg_n1"
DIR_NODE2="/tmp/scytale_reorg_n2"
SOCK_NODE1="/tmp/scytale_reorg_n1.sock"
SOCK_NODE2="/tmp/scytale_reorg_n2.sock"
PORT_P2P_1=9001
PORT_P2P_2=9002

PID_NODE1=""
PID_NODE2=""

cleanup() {
    echo ""
    echo "[!] Membersihkan seluruh proses dan data uji coba..."
    if [ -n "${PID_NODE1}" ] && kill -0 "${PID_NODE1}" 2>/dev/null; then
        kill "${PID_NODE1}" 2>/dev/null || true
    fi
    if [ -n "${PID_NODE2}" ] && kill -0 "${PID_NODE2}" 2>/dev/null; then
        kill "${PID_NODE2}" 2>/dev/null || true
    fi
    pkill -f "scytale-p2p.*${SOCK_NODE1}" 2>/dev/null || true
    pkill -f "scytale-p2p.*${SOCK_NODE2}" 2>/dev/null || true
    rm -rf "${DIR_NODE1}" "${DIR_NODE2}" "${SOCK_NODE1}" "${SOCK_NODE2}" "/tmp/scytale_reorg_*.sock"
    echo "[✓] Lingkungan uji coba telah dibersihkan."
}

trap cleanup EXIT INT TERM

echo "============================================================"
echo "      SCYTALE DYNAMIC PEER & LIVE FORK REORG HARNESS        "
echo "============================================================"

# 1. Kompilasi binary
echo "[1/6] Memeriksa & mengompilasi binary (Rust + Go)..."
cargo build --bin scytale-node --bin scytale-cli
(cd "${ROOT_DIR}/network" && go build -o "${BIN_P2P}" ./cmd/scytale-p2p)

rm -rf "${DIR_NODE1}" "${DIR_NODE2}" "${SOCK_NODE1}" "${SOCK_NODE2}"
mkdir -p "${DIR_NODE1}" "${DIR_NODE2}"

# 2. Jalankan Node 1 dalam partisi terisolasi (tanpa --peer)
echo "[2/6] Menjalankan Node 1 (Port P2P: ${PORT_P2P_1}, IPC: ${SOCK_NODE1})..."
"${BIN_NODE}" \
    --data-dir "${DIR_NODE1}" \
    --socket "${SOCK_NODE1}" \
    start \
    --p2p-bind "127.0.0.1:${PORT_P2P_1}" \
    --p2p-bin "${BIN_P2P}" \
    --target "0x207fffff" \
    --miner-payout "010203" \
    > "/tmp/scytale_reorg_n1.log" 2>&1 &
PID_NODE1=$!

# Tunggu socket Node 1 siap
for i in {1..30}; do
    if [ -S "${SOCK_NODE1}" ]; then break; fi
    sleep 0.1
done
if [ ! -S "${SOCK_NODE1}" ]; then
    echo "[-] Gagal mengaktifkan Node 1. Log:"
    cat /tmp/scytale_reorg_n1.log
    exit 1
fi
echo "     [+] Node 1 aktif (PID: ${PID_NODE1})."

# 3. Jalankan Node 2 dalam partisi terisolasi (tanpa --peer, miner payout beda)
echo "[3/6] Menjalankan Node 2 (Port P2P: ${PORT_P2P_2}, IPC: ${SOCK_NODE2})..."
"${BIN_NODE}" \
    --data-dir "${DIR_NODE2}" \
    --socket "${SOCK_NODE2}" \
    start \
    --p2p-bind "127.0.0.1:${PORT_P2P_2}" \
    --p2p-bin "${BIN_P2P}" \
    --target "0x207fffff" \
    --miner-payout "040506" \
    > "/tmp/scytale_reorg_n2.log" 2>&1 &
PID_NODE2=$!

# Tunggu socket Node 2 siap
for i in {1..30}; do
    if [ -S "${SOCK_NODE2}" ]; then break; fi
    sleep 0.1
done
if [ ! -S "${SOCK_NODE2}" ]; then
    echo "[-] Gagal mengaktifkan Node 2. Log:"
    cat /tmp/scytale_reorg_n2.log
    exit 1
fi
echo "     [+] Node 2 aktif (PID: ${PID_NODE2})."

# 4. Tambang 2 blok pada Node 1 (Cabang A)
echo "[4/6] Menambang rantai minoritas (2 blok) pada Node 1..."
"${BIN_CLI}" --socket "${SOCK_NODE1}" mine --start > /dev/null
while true; do
    H1=$("${BIN_CLI}" --socket "${SOCK_NODE1}" status | grep "Canonical Height" | awk '{print $4}')
    if [ "${H1}" -ge 2 ]; then break; fi
    sleep 0.05
done
"${BIN_CLI}" --socket "${SOCK_NODE1}" mine --stop > /dev/null
H1=$("${BIN_CLI}" --socket "${SOCK_NODE1}" status | grep "Canonical Height" | awk '{print $4}')
TIP1=$("${BIN_CLI}" --socket "${SOCK_NODE1}" status | grep "Canonical Tip" | awk '{print $4}')
echo "     [+] Node 1 Cabang A: Height = ${H1}, Tip = ${TIP1}"

# 5. Tambang rantai mayoritas pada Node 2 (Pastikan Height Node 2 > Node 1)
echo "[5/6] Menambang rantai mayoritas pada Node 2 (Target Height > ${H1})..."
TARGET_H2=$((H1 + 10))
"${BIN_CLI}" --socket "${SOCK_NODE2}" mine --start > /dev/null
while true; do
    H2=$("${BIN_CLI}" --socket "${SOCK_NODE2}" status | grep "Canonical Height" | awk '{print $4}')
    if [ "${H2}" -ge "${TARGET_H2}" ]; then break; fi
    sleep 0.05
done
"${BIN_CLI}" --socket "${SOCK_NODE2}" mine --stop > /dev/null
H2=$("${BIN_CLI}" --socket "${SOCK_NODE2}" status | grep "Canonical Height" | awk '{print $4}')
TIP2=$("${BIN_CLI}" --socket "${SOCK_NODE2}" status | grep "Canonical Tip" | awk '{print $4}')
echo "     [+] Node 2 Cabang B: Height = ${H2} (Mayoritas, Node 1 = ${H1}), Tip = ${TIP2}"

# Verifikasi Partisi / Chain Fork
if [ "${TIP1}" = "${TIP2}" ]; then
    echo "[-] Error: Tip hash kedua node sama sebelum rekoneksi. Partisi gagal!"
    exit 1
fi
echo "     [✓] Terverifikasi: Kedua node berada pada cabang percabangan terpisah (Fork Partisi Aktif)."

# 6. Picu Dynamic Peer Connect dari Node 1 ke Node 2
echo "[6/6] Menghubungkan Node 1 ke Node 2 secara runtime (scytale-cli peer connect)..."
"${BIN_CLI}" --socket "${SOCK_NODE1}" peer connect "127.0.0.1:${PORT_P2P_2}"

echo "     [*] Menunggu proses Initial Block Download & Live Reorganisasi..."
REORG_SUCCESS=false
for i in {1..50}; do
    CUR_H1=$("${BIN_CLI}" --socket "${SOCK_NODE1}" status | grep "Canonical Height" | awk '{print $4}' || echo "0")
    CUR_TIP1=$("${BIN_CLI}" --socket "${SOCK_NODE1}" status | grep "Canonical Tip" | awk '{print $4}' || echo "")
    if [ "${CUR_H1}" -eq "${H2}" ] && [ "${CUR_TIP1}" = "${TIP2}" ]; then
        REORG_SUCCESS=true
        break
    fi
    sleep 0.1
done

echo "============================================================"
echo "HASIL LIVE REORGANISASI:"
echo "Node 1 Final Height : ${CUR_H1} (Awal: ${H1}) | Tip: ${CUR_TIP1}"
echo "Node 2 Final Height : ${H2} | Tip: ${TIP2}"
echo "============================================================"

if [ "${REORG_SUCCESS}" = true ]; then
    echo "[✓] SUKSES: Node 1 berhasil mendeteksi cabang lebih berat, membatalkan cabang lama (Height ${H1}), dan melakukan reorganisasi atomik ke Cabang Node 2 (Height ${H2})!"
else
    echo "[-] GAGAL: Node 1 tidak berhasil melakukan reorganisasi ke tip Node 2."
    echo "Log Node 1:"
    cat /tmp/scytale_reorg_n1.log
    exit 1
fi

echo ""
echo "--- Buku Tabungan (Passbook) Miner 1 (010203) di Node 1 (Cabang Dibatalkan) ---"
"${BIN_CLI}" --socket "${SOCK_NODE1}" passbook --lock "010203"

echo ""
echo "--- Buku Tabungan (Passbook) Miner 2 (040506) di Node 1 (Cabang Dimenangkan) ---"
"${BIN_CLI}" --socket "${SOCK_NODE1}" passbook --lock "040506"
