# Work Record: 44. P2P Network Connection Repair via DNS Seeder (seed.myratu.com)

## Overview
Melakukan diagnosa dan perbaikan konektivitas jaringan P2P pada node Scytale yang berjalan dalam lingkungan Docker di host `ratu-H110`. Memastikan node terhubung ke jaringan publik melalui DNS Seeder resmi `seed.myratu.com` (45.147.46.122:9001) dengan konfigurasi DNS terpercaya (Cloudflare & Google DNS) untuk menghindari interferensi DNS ISP lokal.

## Root Cause Analysis
1. **DNS Seeder Default Usang**:
   Pada `network/cmd/scytale-p2p/main.go`, default fallback DNS seeder masih mengarah ke domain usang `seed.scytale.org` yang me-resolve ke IP `202.61.232.25` (tidak merespons / i/o timeout handshake).
2. **Ketiadaan Parameter DNS Seeder di Compose**:
   Konfigurasi `node-1` pada `docker-compose.yml` belum menyertakan argumen `--dns-seed seed.myratu.com` atau `--peer 45.147.46.122:9001`.
3. **Konfigurasi DNS Docker**:
   Container `node-1`, `node-2`, dan `node-3` belum memiliki directive `dns: [1.1.1.1, 8.8.8.8]`, sehingga bergantung pada resolver host.
4. **Cache Peers Usang (`peers.json`)**:
   File cache `/app/peers.json` di dalam container menyimpan daftar IP lama (`202.61.232.25`) yang terus dihubungi berulang kali.

## Action Plan
1. **Perbarui `apps/scytale-node/src/main.rs`**:
   - Tambahkan alias `#[arg(long = "dns-seed", visible_alias = "seed", action = clap::ArgAction::Append)]` agar mendukung baik `--dns-seed` maupun `--seed`.
   - Tambahkan alias `#[arg(long = "peer", visible_alias = "seed-nodes", action = clap::ArgAction::Append)]` agar mendukung `--peer` dan `--seed-nodes`.
2. **Perbarui Default Fallback di `network/cmd/scytale-p2p/main.go`**:
   - Ubah default fallback `dnsSeedsFlag` menjadi `[]string{"seed.myratu.com"}`.
3. **Perbarui `docker-compose.yml`**:
   - Tambahkan `dns: [1.1.1.1, 8.8.8.8]` pada service node.
   - Tambahkan `--dns-seed seed.myratu.com` dan `--peer seed.myratu.com:9001` pada startup command `node-1`.
4. **Pembersihan Cache & Restart Stack**:
   - Restart container dengan konfigurasi baru.
   - Verifikasi resolusi `seed.myratu.com` -> `45.147.46.122` dari dalam container.
   - Verifikasi pembukaan socket TCP ke `45.147.46.122:9001`.
   - Periksa 50 baris log terakhir container untuk memastikan P2P handshake berhasil.
