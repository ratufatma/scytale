#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_NODE="${ROOT_DIR}/target/release/scytale-node"
BIN_CLI="${ROOT_DIR}/target/release/scytale-cli"
BIN_P2P="${ROOT_DIR}/target/release/scytale-p2p"

DATA_N1="/tmp/scytale_soak_n1"
DATA_N2="/tmp/scytale_soak_n2"
SOCK_N1="/tmp/soak_n1.sock"
SOCK_N2="/tmp/soak_n2.sock"

PORT_P2P_1="9301"
PORT_P2P_2="9302"
PORT_HTTP_1="8341"
PORT_HTTP_2="8342"

DURATION="${DURATION:-60}"          # Durasi dalam detik
CONCURRENCY="${CONCURRENCY:-8}"     # Jumlah worker HTTP concurrent
TELEMETRY_LOG="/tmp/soak_telemetry.csv"

cleanup() {
    echo ""
    echo "[!] Membersihkan seluruh background process..."
    if [ -n "${STRESS_PIDS:-}" ]; then
        for spid in ${STRESS_PIDS}; do
            kill "${spid}" 2>/dev/null || true
        done
    fi
    kill "${PID_METRICS:-}" 2>/dev/null || true
    kill "${PID_N1:-}" "${PID_N2:-}" 2>/dev/null || true
    wait "${PID_N1:-}" "${PID_N2:-}" 2>/dev/null || true
    rm -rf "${DATA_N1}" "${DATA_N2}" "${SOCK_N1}" "${SOCK_N2}"
    echo "[✓] Lingkungan pengujian dibersihkan."
}
trap cleanup EXIT INT TERM

echo "========================================================================"
echo "         SCYTALE PROTOCOL — LONG-RUNNING SOAK & STRESS HARNESS          "
echo "========================================================================"
echo "Target Durasi : ${DURATION} detik"
echo "Konkurensi    : ${CONCURRENCY} HTTP Workers"
echo "Log Telemetri : ${TELEMETRY_LOG}"
echo "========================================================================"

# 1. Kompilasi Profil Release untuk Uji Performa Nyata
echo "[1/6] Mengompilasi binary dengan profil release..."
cargo build --release -p scytale-node -p scytale-cli
(cd "${ROOT_DIR}/network" && go build -ldflags="-s -w" -o "${BIN_P2P}" ./cmd/scytale-p2p)

rm -rf "${DATA_N1}" "${DATA_N2}" "${SOCK_N1}" "${SOCK_N2}" "${TELEMETRY_LOG}"
mkdir -p "${DATA_N1}" "${DATA_N2}"

# 2. Jalankan Node 1 (Miner + HTTP) & Node 2 (Follower)
echo "[2/6] Memulai Node 1 (Port P2P: ${PORT_P2P_1}, HTTP: ${PORT_HTTP_1})..."
"${BIN_NODE}" --data-dir "${DATA_N1}" --socket "${SOCK_N1}" start \
    --p2p-bind "127.0.0.1:${PORT_P2P_1}" \
    --p2p-bin "${BIN_P2P}" \
    --http-bind "127.0.0.1:${PORT_HTTP_1}" \
    --target 0x207fffff > "/tmp/soak_n1.log" 2>&1 &
PID_N1=$!

echo "      Memulai Node 2 (Port P2P: ${PORT_P2P_2}, HTTP: ${PORT_HTTP_2}, Peer: Node 1)..."
"${BIN_NODE}" --data-dir "${DATA_N2}" --socket "${SOCK_N2}" start \
    --p2p-bind "127.0.0.1:${PORT_P2P_2}" \
    --p2p-bin "${BIN_P2P}" \
    --http-bind "127.0.0.1:${PORT_HTTP_2}" \
    --peer "127.0.0.1:${PORT_P2P_1}" \
    --target 0x207fffff > "/tmp/soak_n2.log" 2>&1 &
PID_N2=$!

# Tunggu socket dan gateway siap
for _ in {1..30}; do
    if [ -S "${SOCK_N1}" ] && [ -S "${SOCK_N2}" ] && curl -s "http://127.0.0.1:${PORT_HTTP_1}/health" >/dev/null 2>&1; then
        break
    fi
    sleep 0.2
done
echo "      [+] Kedua node beroperasi normal. Node 1 PID: ${PID_N1}, Node 2 PID: ${PID_N2}"

# 3. Aktifkan Penambangan Otonom di Node 1
echo "[3/6] Mengaktifkan autonomous PoW mining di Node 1..."
"${BIN_CLI}" --socket "${SOCK_N1}" mine --start

# 4. Spawn Background Telemetry Logger
echo "elapsed_sec,canonical_height,rss_kb,open_fds,db_size_kb" > "${TELEMETRY_LOG}"
record_metrics() {
    local start_time
    start_time=$(date +%s)
    while true; do
        local now elapsed height rss fds db_size
        now=$(date +%s)
        elapsed=$((now - start_time))
        
        # Ambil Ketinggian Blok via HTTP
        height=$(curl -s "http://127.0.0.1:${PORT_HTTP_1}/api/v1/status" 2>/dev/null | grep -o '"canonical_height":[0-9]*' | cut -d':' -f2 || echo "0")
        
        # Ambil RSS (KB) Node 1
        rss=$(ps -o rss= -p "${PID_N1}" 2>/dev/null | tr -d ' ' || echo "0")
        
        # Ambil Open FDs Node 1
        if [ -d "/proc/${PID_N1}/fd" ]; then
            fds=$(ls -1 "/proc/${PID_N1}/fd" 2>/dev/null | wc -l)
        else
            fds=$(lsof -p "${PID_N1}" 2>/dev/null | wc -l)
        fi

        # Ukuran file database redb
        db_size=$(du -k "${DATA_N1}/scytale.db" 2>/dev/null | awk '{print $1}' || echo "0")
        
        echo "${elapsed},${height},${rss},${fds},${db_size}" >> "${TELEMETRY_LOG}"
        sleep 2
    done
}
record_metrics &
PID_METRICS=$!

# 5. Bombardir dengan HTTP Concurrent Traffic
echo "[4/6] Menjalankan ${CONCURRENCY} HTTP stress worker paralel selama ${DURATION} detik..."
STRESS_PIDS=""
for i in $(seq 1 "${CONCURRENCY}"); do
    (
        END_TIME=$(( $(date +%s) + DURATION ))
        while [ "$(date +%s)" -lt "${END_TIME}" ]; do
            curl -s "http://127.0.0.1:${PORT_HTTP_1}/api/v1/status" >/dev/null 2>&1 || true
            curl -s "http://127.0.0.1:${PORT_HTTP_1}/api/v1/blocks/tip" >/dev/null 2>&1 || true
            curl -s "http://127.0.0.1:${PORT_HTTP_1}/api/v1/passbook/010203" >/dev/null 2>&1 || true
        done
    ) &
    STRESS_PIDS="${STRESS_PIDS} $!"
done

# Hitung mundur
for sec in $(seq "${DURATION}" -1 1); do
    if [ $((sec % 10)) -eq 0 ] || [ "${sec}" -le 5 ]; then
        LATEST_METRIC=$(tail -n 1 "${TELEMETRY_LOG}")
        echo "      [*] Sisa waktu: ${sec}s | Terakhir: ${LATEST_METRIC}"
    fi
    sleep 1
done

# Tunggu stress workers selesai
for spid in ${STRESS_PIDS}; do
    wait "${spid}" 2>/dev/null || true
done
kill "${PID_METRICS}" 2>/dev/null || true

# Hentikan Mining
"${BIN_CLI}" --socket "${SOCK_N1}" mine --stop
sleep 2

# 6. Evaluasi Invarian Konsensus & Telemetri
echo "[5/6] Menganalisis hasil telemetri dan kestabilan konsensus..."

# Evaluasi Sinkronisasi Node 2 (Follower)
STATUS_N1=$("${BIN_CLI}" --socket "${SOCK_N1}" status)
HEIGHT_1=$(echo "${STATUS_N1}" | grep -i "Canonical Height" | awk '{print $NF}')
TIP_1=$(echo "${STATUS_N1}" | grep -i "Canonical Tip" | awk '{print $NF}')

for _ in {1..150}; do
    STATUS_N2=$("${BIN_CLI}" --socket "${SOCK_N2}" status)
    HEIGHT_2=$(echo "${STATUS_N2}" | grep -i "Canonical Height" | awk '{print $NF}')
    TIP_2=$(echo "${STATUS_N2}" | grep -i "Canonical Tip" | awk '{print $NF}')
    if [ "${HEIGHT_2}" -eq "${HEIGHT_1}" ] && [ "${TIP_2}" == "${TIP_1}" ]; then
        break
    fi
    sleep 0.2
done

echo "------------------------------------------------------------------------"
echo "KONSENSUS FINAL:"
echo "Node 1: Height = ${HEIGHT_1} | Tip = ${TIP_1}"
echo "Node 2: Height = ${HEIGHT_2} | Tip = ${TIP_2}"
echo "------------------------------------------------------------------------"

if [ "${HEIGHT_1}" -ne "${HEIGHT_2}" ] || [ "${TIP_1}" != "${TIP_2}" ]; then
    echo "[-] GAGAL: Node 2 gagal menyinkronkan seluruh blok dari Node 1 di bawah beban!"
    exit 1
fi
echo "[✓] P2P Invariant PASS: Node 2 tersinkronisasi 100% dengan Node 1."

# Evaluasi RSS Drift (Memori)
WARMUP_RSS=$(awk -F',' 'NR==3 {print $3}' "${TELEMETRY_LOG}")
FINAL_LINE=$(tail -n 1 "${TELEMETRY_LOG}")
FINAL_RSS=$(echo "${FINAL_LINE}" | awk -F',' '{print $3}')
FINAL_FDS=$(echo "${FINAL_LINE}" | awk -F',' '{print $4}')
FINAL_DB_KB=$(echo "${FINAL_LINE}" | awk -F',' '{print $5}')

WARMUP_RSS=${WARMUP_RSS:-0}
FINAL_RSS=${FINAL_RSS:-0}
WARMUP_MB=$(( WARMUP_RSS / 1024 ))
FINAL_MB=$(( FINAL_RSS / 1024 ))
RSS_DIFF_MB=$(( FINAL_MB - WARMUP_MB ))

echo "METRIK TELEMETRI NODE 1:"
echo "Warmup RSS : ${WARMUP_MB} MB"
echo "Final RSS  : ${FINAL_MB} MB"
echo "Delta RSS  : ${RSS_DIFF_MB} MB"
echo "Open FDs   : ${FINAL_FDS}"
echo "DB Size    : ${FINAL_DB_KB} KB"

if [ "${RSS_DIFF_MB}" -gt 50 ]; then
    echo "[-] GAGAL: Terdeteksi lonjakan memori abnormal (> 50 MB) mengindikasikan leak."
    exit 1
fi
echo "[✓] Memory Leak Invariant PASS: Penggunaan memori stabil."

# 7. Graceful Shutdown & Database Integrity Re-open Check
echo "[6/6] Memverifikasi integritas database redb pasca-shutdown..."
"${BIN_CLI}" --socket "${SOCK_N1}" stop
"${BIN_CLI}" --socket "${SOCK_N2}" stop
wait "${PID_N1}" "${PID_N2}" 2>/dev/null || true
rm -f "${SOCK_N1}" "${SOCK_N2}"
sleep 1

# Buka ulang database Node 1 untuk memastikan tidak ada B-Tree corruption
echo "      [*] Membuka ulang basis data Node 1 (${HEIGHT_1} blok)..."
"${BIN_NODE}" --data-dir "${DATA_N1}" --socket "${SOCK_N1}" start --no-p2p --no-http > "/tmp/verify_db.log" 2>&1 &
VERIFY_PID=$!

VERIFY_STATUS=""
for _ in {1..300}; do
    if "${BIN_CLI}" --socket "${SOCK_N1}" status > "/tmp/vstatus.txt" 2>/dev/null; then
        VERIFY_STATUS=$(cat "/tmp/vstatus.txt")
        rm -f "/tmp/vstatus.txt"
        break
    fi
    sleep 0.2
done

VERIFY_HEIGHT=$(echo "${VERIFY_STATUS}" | grep -i "Canonical Height" | awk '{print $NF}' || echo "")

"${BIN_CLI}" --socket "${SOCK_N1}" stop 2>/dev/null || true
wait "${VERIFY_PID}" 2>/dev/null || true

if [ -n "${VERIFY_HEIGHT}" ] && [ "${VERIFY_HEIGHT}" -eq "${HEIGHT_1}" ]; then
    echo "[✓] Storage Integrity PASS: Basis data redb pulih sempurna pada Height ${VERIFY_HEIGHT}."
else
    echo "[-] GAGAL: Ketinggian basis data tidak konsisten setelah dimuat ulang (dapat: '${VERIFY_HEIGHT}', ekspektasi: '${HEIGHT_1}')!"
    exit 1
fi

echo "========================================================================"
echo "    HASIL UJI: SELURUH INVARIAN SOAK & STRESS TEST TERBUKTI STABIL      "
echo "========================================================================"