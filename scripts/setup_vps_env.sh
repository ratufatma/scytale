#!/usr/bin/env bash
# ==============================================================================
# Scytale Production VPS Environment Provisioning & Hardening Script
# ==============================================================================
# Mengatur direktori kerja, user non-root scytale, hak akses storage (0700),
# template environment variables, unit systemd, dan firewall UFW.
# ==============================================================================

set -euo pipefail

# Verifikasi akses root
if [ "$EUID" -ne 0 ]; then
    echo "[-] Skrip ini harus dijalankan dengan hak akses root / sudo." >&2
    exit 1
fi

echo "========================================================================"
echo " ==> Memulai Setup Lingkungan VPS Scytale Blockchain"
echo "========================================================================"

SCY_USER="scytale"
SCY_GROUP="scytale"
DATA_BASE_DIR="/var/lib/scytale"
DATA_STORAGE_DIR="/var/lib/scytale/data"
EXPLORER_DIR="/var/www/scytale-explorer"
CONFIG_DIR="/etc/scytale"
RUN_DIR="/run/scytale"
SYSTEMD_DIR="/etc/systemd/system"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 1. Pembuatan System User & Group Non-Root
echo "[+] 1/6 Memastikan system user dan group '${SCY_USER}'..."
if ! getent group "${SCY_GROUP}" >/dev/null 2>&1; then
    groupadd --system "${SCY_GROUP}"
    echo "    Group '${SCY_GROUP}' berhasil dibuat."
fi

if ! getent passwd "${SCY_USER}" >/dev/null 2>&1; then
    useradd --system \
        --gid "${SCY_GROUP}" \
        --home-dir "${DATA_BASE_DIR}" \
        --shell /usr/sbin/nologin \
        --comment "Scytale Blockchain Daemon System Account" \
        "${SCY_USER}"
    echo "    User '${SCY_USER}' berhasil dibuat."
fi

# 2. Pembuatan Hirarki Direktori
echo "[+] 2/6 Menyiapkan direktori sistem..."
mkdir -p "${DATA_STORAGE_DIR}"
mkdir -p "${EXPLORER_DIR}"
mkdir -p "${CONFIG_DIR}"
mkdir -p "${RUN_DIR}"

# 3. Hak Akses Storage & Sandboxing (0700 untuk data sensitif)
echo "[+] 3/6 Menerapkan hak akses ketat (0700 pada database storage)..."
chown -R "${SCY_USER}:${SCY_GROUP}" "${DATA_BASE_DIR}"
chown -R "${SCY_USER}:${SCY_GROUP}" "${EXPLORER_DIR}"
chown -R "${SCY_USER}:${SCY_GROUP}" "${CONFIG_DIR}"
chown -R "${SCY_USER}:${SCY_GROUP}" "${RUN_DIR}"

chmod 0750 "${DATA_BASE_DIR}"
chmod 0700 "${DATA_STORAGE_DIR}"     # Ketat: hanya user scytale yang dapat membaca redb
chmod 0750 "${EXPLORER_DIR}"
chmod 0700 "${CONFIG_DIR}"           # Ketat: memuat token autentikasi
chmod 0755 "${RUN_DIR}"

# 4. Template Environment Variables & Ingest Auth Token
echo "[+] 4/6 Menyiapkan berkas environment (/etc/scytale)..."
SHARED_TOKEN=$(openssl rand -hex 24 2>/dev/null || echo "scytale_auth_$(date +%s)")

if [ ! -f "${CONFIG_DIR}/node.env" ]; then
    cat <<EOF > "${CONFIG_DIR}/node.env"
# Environment configuration for Scytale Node Daemon
INDEXER_KEY=${SHARED_TOKEN}
NODE_EXTRA_ARGS="--indexer-key ${SHARED_TOKEN}"
EOF
    chmod 0600 "${CONFIG_DIR}/node.env"
    chown "${SCY_USER}:${SCY_GROUP}" "${CONFIG_DIR}/node.env"
    echo "    Dibuat: ${CONFIG_DIR}/node.env"
fi

if [ ! -f "${CONFIG_DIR}/explorer.env" ]; then
    cat <<EOF > "${CONFIG_DIR}/explorer.env"
# Environment configuration for Scytale Web Explorer
INDEXER_KEY=${SHARED_TOKEN}
EXPLORER_API_KEY=${SHARED_TOKEN}
NODE_URL=http://127.0.0.1:8332
HOST=127.0.0.1
PORT=3000
EXPLORER_DB_PATH=${DATA_BASE_DIR}/explorer.db
EOF
    chmod 0600 "${CONFIG_DIR}/explorer.env"
    chown "${SCY_USER}:${SCY_GROUP}" "${CONFIG_DIR}/explorer.env"
    echo "    Dibuat: ${CONFIG_DIR}/explorer.env"
fi

# 5. Pemasangan Unit Systemd
echo "[+] 5/6 Memasang template unit service systemd..."
if [ -d "${SCRIPT_DIR}/systemd" ]; then
    cp -f "${SCRIPT_DIR}/systemd/scytale-node.service" "${SYSTEMD_DIR}/"
    cp -f "${SCRIPT_DIR}/systemd/scytale-explorer.service" "${SYSTEMD_DIR}/"
    chmod 0644 "${SYSTEMD_DIR}/scytale-node.service" "${SYSTEMD_DIR}/scytale-explorer.service"
    systemctl daemon-reload || true
    echo "    Unit systemd scytale-node dan scytale-explorer berhasil didaftarkan."
fi

# 6. Konfigurasi Firewall UFW
echo "[+] 6/6 Memeriksa konfigurasi firewall UFW..."
if command -v ufw >/dev/null 2>&1; then
    echo "    Mengonfigurasi aturan UFW:"
    ufw allow 22/tcp comment "SSH Remote Management" || true
    ufw allow 9001/tcp comment "Scytale P2P Consensus Wire Protocol" || true
    # Port 8332 (Node Gateway) dan 3000 (Explorer) sengaja tidak diekspos publik,
    # hanya diakses lokal atau di-reverse proxy via Nginx/Caddy/Cloudflare Tunnel.
    echo "    - Port 22/tcp (SSH): ALLOW"
    echo "    - Port 9001/tcp (P2P Wire): ALLOW"
    echo "    - Port 8332 & 3000 (Internal API & Explorer): STRICT LOCALHOST (PROTECTED)"
else
    echo "    [i] UFW tidak terpasang di sistem, lewati konfigurasi otomatis firewall."
fi

echo "========================================================================"
echo " ==> Setup Lingkungan VPS Scytale Berhasil Diselesaikan!"
echo "     - Binari Node diharapkan berada di: /usr/local/bin/scytale-node"
echo "     - Frontend Explorer di: /var/www/scytale-explorer"
echo "     - Jalankan service dengan:"
echo "       sudo systemctl enable --now scytale-node"
echo "       sudo systemctl enable --now scytale-explorer"
echo "========================================================================"
