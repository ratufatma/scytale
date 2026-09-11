#!/usr/bin/env bash
set -e
VPS_HOST="${VPS_HOST:?VPS_HOST must be set}"
VPS_USER="${VPS_USER:?VPS_USER must be set}"
DEST_DIR="${DEST_DIR:-/var/www/scytale-explorer/}"

cd "$(dirname "$0")/../explorer"
echo "==> Memulai build frontend explorer..."
npm run build

BUILD_DIR="out"
[ -d "dist" ] && BUILD_DIR="dist"

echo "==> Mengunggah file statis ke VPS ($VPS_HOST:$DEST_DIR)..."
rsync -avz --delete "$BUILD_DIR/" "$VPS_USER@$VPS_HOST:$DEST_DIR"
echo "==> Deployment explorer berhasil!"
