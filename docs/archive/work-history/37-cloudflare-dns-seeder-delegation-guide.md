# Scytale Protocol — Technical Specification & Architecture Record: Task 37
## Cloudflare DNS & NS Delegation Guide for Autonomous DNS Seeder

```text
Task ID       : 37
Task Name     : Cloudflare DNS & NS Delegation Guide for Autonomous DNS Seeder
Phase         : Phase 4 — Network Bootstrap & Production Tooling
Target Files  : docs/DNS-SEEDER-DEPLOYMENT-GUIDE.md, docs/work/37-cloudflare-dns-seeder-delegation-guide.md
Reference     : network/cmd/scytale-seeder, network/internal/seeder
Status        : COMPLETED / PRODUCTION-READY
Invariants    : Authoritative NS Delegation, Grey Cloud (DNS Only), Dual-Stack (IPv4/IPv6), Low TTL (60s), Zero Proxy Interception
```

---

## 1. Arsitektur Pendelegasian DNS Seeder

Untuk menghubungkan daemon `scytale-seeder` yang berjalan di VPS/Server publik ke ekosistem internet global, domain `seed.scytale.org` harus didelegasikan dari penyedia DNS utama (Cloudflare) ke nameserver otoritatif mandiri:

```text
┌────────────────────────────────────────────────────────────────────────┐
│                          GLOBAL DNS RESOLVER                           │
│                      (1.1.1.1 / 8.8.8.8 / Local ISP)                   │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                         1. Kueri: A seed.scytale.org
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                        CLOUDFLARE DNS DASHBOARD                        │
│                          (Zona: scytale.org)                           │
│                                                                        │
│   Record 1 (Glue A):                                                   │
│   • Type: A                                                            │
│   • Name: ns1.seed                                                     │
│   • Target: 203.0.113.10 (IP Publik VPS Seeder)                        │
│   • Proxy: DNS Only (Grey Cloud) ◄─── KRITIS: TIDAK BOLEH ORANGE!      │
│                                                                        │
│   Record 2 (Delegasi NS):                                              │
│   • Type: NS                                                           │
│   • Name: seed                                                         │
│   • Target: ns1.seed.scytale.org                                       │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                         2. Rujukan: NS ns1.seed.scytale.org
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                     SERVER SCYTALE SEEDER DAEMON                       │
│                     (203.0.113.10:53 UDP & TCP)                        │
│                                                                        │
│   • Membaca kueri masuk: Type A / AAAA seed.scytale.org                │
│   • Mengambil daftar "Good Nodes" dari Memory Store                    │
│   • Mengacak daftar node (Fisher-Yates) & membatasi 16 IP              │
│   • Menjawab dengan Authoritative = true, TTL 60 detik                 │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Langkah Demi Langkah Konfigurasi Cloudflare DNS

### Langkah 1: Siapkan Server VPS & IP Publik
* Catat alamat IP publik IPv4 dan IPv6 server tempat `scytale-seeder` dijalankan:
  * IPv4: misal `203.0.113.10`
  * IPv6 (opsional): misal `2001:db8::10`

### Langkah 2: Tambahkan Glue Record (A & AAAA Record)
Glue record diperlukan agar resolver DNS mengetahui alamat IP dari nameserver `ns1.seed.scytale.org`:
1. Masuk ke **Cloudflare Dashboard** $\rightarrow$ Pilih domain **`scytale.org`** $\rightarrow$ Menu **DNS** $\rightarrow$ **Records**.
2. Klik **Add record**:
   * **Type**: `A`
   * **Name**: `ns1.seed` *(akan menjadi `ns1.seed.scytale.org`)*
   * **IPv4 address**: `203.0.113.10` *(sesuaikan dengan IP VPS Anda)*
   * **Proxy status**: **DNS only** *(Pastikan ikon awan berwarna ABU-ABU / Grey Cloud)*
   * **TTL**: `Auto`
3. *(Opsional untuk IPv6)* Tambahkan record AAAA:
   * **Type**: `AAAA`
   * **Name**: `ns1.seed`
   * **IPv6 address**: `2001:db8::10`
   * **Proxy status**: **DNS only**

> [!CAUTION]
> **JANGAN PERNAH** mengaktifkan Proxy Cloudflare (Orange Cloud) pada record ini. Cloudflare Proxy hanya mendukung protokol web HTTP/HTTPS (port 80/443). Mengaktifkan orange cloud akan memblokir paket UDP/TCP port 53 dan merusak seeder sepenuhnya.

### Langkah 3: Tambahkan NS Delegation Record
Delegasikan subdomain `seed` ke nameserver yang baru dibuat:
1. Klik **Add record**:
   * **Type**: `NS`
   * **Name**: `seed` *(akan menjadi `seed.scytale.org`)*
   * **Nameserver**: `ns1.seed.scytale.org`
   * **TTL**: `Auto`
2. Simpan record.

---

## 3. Konfigurasi Server Linux & Firewall

### A. Buka Port Firewall (UFW / iptables)
Server harus menerima lalu lintas DNS masuk pada port 53 (UDP & TCP) serta port P2P (9001):
```bash
# Izinkan DNS port 53 (UDP & TCP)
sudo ufw allow 53/udp
sudo ufw allow 53/tcp

# Izinkan Scytale P2P crawler outbound/inbound
sudo ufw allow 9001/tcp

# Muat ulang firewall
sudo ufw reload
```

### B. Nonaktifkan `systemd-resolved` Stub Listener (Jika Ada)
Secara default, Ubuntu menyalakan resolver internal pada `127.0.0.53:53` yang dapat menyebabkan konflik port:
```bash
# Periksa apakah port 53 sudah terpakai
sudo lsof -i :53

# Jika dipakai systemd-resolved, matikan stub listener:
sudo sed -i 's/#DNSStubListener=yes/DNSStubListener=no/' /etc/systemd/resolved.conf
sudo systemctl restart systemd-resolved
```

### C. Pasang Service Daemon Systemd
Buat file service di `/etc/systemd/system/scytale-seeder.service`:
```ini
[Unit]
Description=Scytale Autonomous DNS Seeder
After=network.target

[Service]
Type=simple
User=scytale
Group=scytale
WorkingDirectory=/home/scytale
ExecStart=/usr/local/bin/scytale-seeder \
    --domain=seed.scytale.org \
    --nameserver=ns1.seed.scytale.org \
    --listen=:53 \
    --p2p-port=9001 \
    --seeds=172.28.0.10:9001,node1.scytale.org:9001 \
    --data-file=/var/lib/scytale/seeder_nodes.json \
    --workers=16 \
    --probe-interval=15m
Restart=always
RestartSec=10s
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE

[Install]
WantedBy=multi-user.target
```

Nyalakan service:
```bash
sudo systemctl daemon-reload
sudo systemctl enable --now scytale-seeder
sudo systemctl status scytale-seeder
```

---

## 4. Panduan Pengujian & Verifikasi

### A. Pengujian Langsung ke Server Seeder
Jalankan `dig` langsung mengarah ke IP publik server Anda:
```bash
dig @203.0.113.10 seed.scytale.org A
```
**Respons yang Diharapkan:**
```text
;; ->>HEADER<<- opcode: QUERY, status: NOERROR, id: 48123
;; flags: qr aa rd; QUERY: 1, ANSWER: 4, AUTHORITY: 0, ADDITIONAL: 0
;; QUESTION SECTION:
;seed.scytale.org.              IN      A

;; ANSWER SECTION:
seed.scytale.org.       60      IN      A       198.51.100.42
seed.scytale.org.       60      IN      A       203.0.113.55

;; Query time: 1 msec
;; SERVER: 203.0.113.10#53(203.0.113.10)
```
*Catatan:* Pastikan bendera `aa` (*Authoritative Answer*) muncul dan TTL adalah `60`.

### B. Pengujian Kueri Otoritatif NS
```bash
dig @203.0.113.10 seed.scytale.org NS
```
**Respons yang Diharapkan:**
```text
;; ANSWER SECTION:
seed.scytale.org.       60      IN      NS      ns1.seed.scytale.org.
```

### C. Pengujian Penelusuran Rekursif Global (Trace)
Jalankan verifikasi delegasi penuh dari DNS root internet:
```bash
dig +trace seed.scytale.org A
```
Pastikan rantai delegasi berjalan mulus:
1. Root DNS (`.`) $\rightarrow$
2. TLD Nameserver (`.org`) $\rightarrow$
3. Cloudflare Nameserver (`scytale.org`) $\rightarrow$
4. Seeder Nameserver (`ns1.seed.scytale.org` / `203.0.113.10`) $\rightarrow$
5. Hasil daftar IP node Scytale.

### D. Pengujian dari Resolver Publik
```bash
# Cloudflare DNS
dig @1.1.1.1 seed.scytale.org A

# Google DNS
dig @8.8.8.8 seed.scytale.org A
```
Kedua resolver publik akan mengembalikan daftar alamat IP aktif node Scytale secara acak dalam kurun waktu $\le 60$ detik.
