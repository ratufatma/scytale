#!/usr/bin/env bash
# ==============================================================================
# Scytale Dockerized 3-Node Cluster Validation Harness
# ==============================================================================
# Skenario:
# 1. Build image Docker multi-stage dan jalankan kluster 3 node (Node 1, Node 2, Node 3).
# 2. Node 1 = Bootstrap node (port 9001).
# 3. Node 2 = Miner node (terhubung ke node1:9001).
# 4. Node 3 = Follower node (terhubung ke node1:9001).
# 5. Biarkan Node 2 menambang blok dan propagasi via jaringan virtual Docker.
# 6. Buktikan bahwa Node 1, Node 2, dan Node 3 memiliki Height dan Tip Hash identik 100%.
# ==============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

cleanup() {
    echo ""
    echo "[!] Menghentikan dan membersihkan kluster Docker..."
    docker compose down -v --remove-orphans 2>/dev/null || true
    echo "[✓] Kluster Docker telah dibersihkan."
}

trap cleanup EXIT INT TERM

echo "============================================================"
echo "         SCYTALE 3-NODE DOCKER CLUSTER VALIDATION           "
echo "============================================================"

# 1. Build & Run Docker Cluster
echo "[1/5] Membangun image Docker & menyalakan kluster 3-node..."
docker compose up -d --build

# 2. Tunggu IPC socket di ketiga container aktif
echo "[2/5] Menunggu ketiga container siap..."
for node in scytale-node1 scytale-node2 scytale-node3; do
    echo -n "     [*] Menunggu socket di container ${node}..."
    READY=false
    for i in {1..60}; do
        if docker exec "${node}" test -S /run/scytale/node.sock 2>/dev/null; then
            READY=true
            break
        fi
        sleep 0.5
    done
    if [ "${READY}" != true ]; then
        echo " FAILED!"
        echo "[-] Container ${node} log:"
        docker logs "${node}"
        exit 1
    fi
    echo " READY."
done

# 3. Biarkan Node 2 menambang blok selama 3 detik
echo "[3/5] Mengamati penambangan blok di Node 2 (Miner Node)..."
sleep 3

# 4. Kumpulkan status konsensus dari ketiga node
echo "[4/5] Mengambil status konsensus dari seluruh node..."
STATUS1=$(docker exec scytale-node1 scytale-cli --socket /run/scytale/node.sock status)
STATUS2=$(docker exec scytale-node2 scytale-cli --socket /run/scytale/node.sock status)
STATUS3=$(docker exec scytale-node3 scytale-cli --socket /run/scytale/node.sock status)

H1=$(echo "${STATUS1}" | grep "Canonical Height" | awk '{print $4}')
TIP1=$(echo "${STATUS1}" | grep "Canonical Tip" | awk '{print $4}')

H2=$(echo "${STATUS2}" | grep "Canonical Height" | awk '{print $4}')
TIP2=$(echo "${STATUS2}" | grep "Canonical Tip" | awk '{print $4}')

H3=$(echo "${STATUS3}" | grep "Canonical Height" | awk '{print $4}')
TIP3=$(echo "${STATUS3}" | grep "Canonical Tip" | awk '{print $4}')

echo "============================================================"
echo "KONSENSUS KLUSTER DOCKER (3 NODE):"
echo "Node 1 (Bootstrap) : Height = ${H1} | Tip = ${TIP1}"
echo "Node 2 (Miner)     : Height = ${H2} | Tip = ${TIP2}"
echo "Node 3 (Follower)  : Height = ${H3} | Tip = ${TIP3}"
echo "============================================================"

# 5. Verifikasi Konsensus 100%
if [ "${H2}" -lt 5 ]; then
    echo "[-] Error: Node 2 gagal menambang minimal 5 blok (Height saat ini: ${H2})."
    exit 1
fi

if [ "${H1}" -ne "${H2}" ] || [ "${H3}" -ne "${H2}" ] || [ "${TIP1}" != "${TIP2}" ] || [ "${TIP3}" != "${TIP2}" ]; then
    echo "[-] GAGAL: Status konsensus antar node belum tersinkronisasi 100%!"
    exit 1
fi

echo "[✓] SUKSES: Seluruh 3 node di kluster Docker tersinkronisasi 100% pada Height ${H2}!"
echo ""
echo "--- Buku Tabungan (Passbook) Hasil Tambang di Node 3 (Follower) ---"
docker exec scytale-node3 scytale-cli --socket /run/scytale/node.sock passbook --lock "010203"
