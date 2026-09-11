# Scytale Web Explorer & Indexer Reference

Dokumen ini menjelaskan arsitektur penjelajah blok web (Web Explorer), ingestion worker metadata blok, dan setup reverse proxy publik Scytale.

---

## 1. Arsitektur Komponen

```text
┌─────────────────┐   HTTP Ingest (POST)    ┌─────────────────────────┐
│  scytale-node   │ ──────────────────────> │  scytale-explorer       │
│  (Port 8332)    │   Header: X-Indexer-Key │  (Port 3000 Node.js)    │
└─────────────────┘                         └───────────┬─────────────┘
                                                        │ Local Proxy
                                                        v
┌─────────────────┐      HTTPS (Port 443)   ┌─────────────────────────┐
│  Browser Publik │ <────────────────────── │  Nginx Reverse Proxy    │
│                 │   explorer.myratu.com   │  (Rate Limit + SSL)     │
└─────────────────┘                         └─────────────────────────┘
```

---

## 2. Ingestion Worker (`POST /api/ingest`)

Ketika node memvalidasi dan meng-commit blok baru ke rantai kanonikal, modul internal `indexer` secara otomatis mem-push metadata blok ke Explorer backend:
- **Target URL**: `http://127.0.0.1:3000/api/ingest`
- **Header Keamanan**: `X-Indexer-Key: <SECRET_KEY>`
- **Payload**: JSON berisi nomor ketinggian (`height`), `hash`, `previous_block_hash`, `miner`, `timestamp`, `tx_count`, dan detail transaksi.

---

## 3. Deployment Publik

- **URL Publik**: [https://explorer.myratu.com](https://explorer.myratu.com)
- **Proksi Terbalik**: Nginx dengan sertifikat Let's Encrypt ECDSA valid.
- **Proteksi API**:
  - Rate limiting otomatis untuk mencegah serangan DDoS (`limit_req`).
  - Restriksi operasi penulisan (`/api/v1/tx`) hanya mengizinkan metode `GET` dan `OPTIONS` dari publik jika diperlukan.
