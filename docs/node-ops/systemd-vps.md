# Panduan Setup Service Produksi Linux / VPS (systemd)

Dokumen ini memuat panduan deployment daemon `scytale-node` sebagai layanan background persisten menggunakan `systemd` dengan pengerasan keamanan (*security hardening*).

---

## 1. Pembuatan User & Direktori Khusus

Jalankan perintah berikut pada server Linux (VPS):

```bash
# Buat pengguna sistem tanpa akses login shell
sudo useradd -r -s /bin/false -d /var/lib/scytale scytale

# Siapkan direktori data dan izin akses
sudo mkdir -p /var/lib/scytale/data /etc/scytale /run/scytale
sudo chown -R scytale:scytale /var/lib/scytale /run/scytale
sudo chmod 700 /var/lib/scytale/data
```

Salin biner rilis ke `/usr/local/bin/`:
```bash
sudo cp target/release/scytale-node /usr/local/bin/
sudo chmod 755 /usr/local/bin/scytale-node
```

---

## 2. Definisi Unit File systemd

Buat file `/etc/systemd/system/scytale-node.service`:

```ini
[Unit]
Description=Scytale Full Node Daemon
Documentation=https://github.com/scytale-lab/scytale
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=scytale
Group=scytale
WorkingDirectory=/var/lib/scytale
RuntimeDirectory=scytale
RuntimeDirectoryMode=0755

EnvironmentFile=-/etc/scytale/node.env

ExecStart=/usr/local/bin/scytale-node \
    --data-dir /var/lib/scytale/data \
    --socket /run/scytale/node.sock \
    start \
    --http-bind 127.0.0.1:8332 \
    --explorer-url http://127.0.0.1:3000/api/ingest \
    $NODE_EXTRA_ARGS

Restart=on-failure
RestartSec=5s

# Limit proses dan deskriptor berkas
LimitNOFILE=65535
LimitNPROC=32768

# Pengerasan keamanan (Sandboxing)
NoNewPrivileges=true
ProtectSystem=full
ProtectHome=true
PrivateTmp=true
ProtectControlGroups=true
ProtectKernelModules=true

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=scytale-node

[Install]
WantedBy=multi-user.target
```

---

## 3. Menjalankan & Memantau Layanan

```bash
# Reload daemon systemd
sudo systemctl daemon-reload

# Aktifkan auto-start saat booting dan jalankan service
sudo systemctl enable --now scytale-node

# Pantau log secara real-time
sudo journalctl -u scytale-node -f
```
