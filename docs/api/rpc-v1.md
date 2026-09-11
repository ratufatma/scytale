# Scytale HTTP RPC v1 API Reference

Gateway HTTP Scytale menyediakan antarmuka REST read-only dan endpoint broadcast transaksi:
- **Port Lokal Default**: `8332` (`http://127.0.0.1:8332`)
- **Akses Publik Aman**: `https://explorer.myratu.com/` (Reverse Proxy Nginx)

---

## 1. `GET /api/v1/status`
Mengembalikan status sinkronisasi, tinggi rantai, tip hash saat ini, dan status penambangan node.

### Contoh Respons (200 OK):
```json
{
  "runtime_state": "Running",
  "canonical_height": 1,
  "canonical_tip": "0x0000000074857748e2050bff75595a287e1e7b507d2a4dcfabc9db4ee70ef552",
  "utxo_root": "0xc9fc38f76aa898bc68ea6b4014d0f0e08b9bce9aab102113e8c6eb5bd1db85c8",
  "peer_count": 1,
  "mempool_tx_count": 0,
  "mining_active": false
}
```

---

## 2. `GET /api/v1/blocks/tip`
Mengembalikan detail penuh dari blok tertinggi kanonikal saat ini.

### Contoh Respons (200 OK):
```json
{
  "hash": "0x0000000074857748e2050bff75595a287e1e7b507d2a4dcfabc9db4ee70ef552",
  "height": 1,
  "version": 1,
  "previous_block_hash": "0x4033f099ae89051a629c871e9af28a215898ff345505ffdbbce65c27a29585c9",
  "transaction_commitment": "0xad4eb13bd80ec9a3ec8f4cad4fd776c8ac430d5a80a115d5c2050ec58c13e976",
  "timestamp": 1789133162,
  "difficulty_target": "0x1d00ffff",
  "nonce": 4820336845,
  "tx_count": 1,
  "transactions": [
    {
      "txid": "0x5bdeb33aa18f8a2eab8172726dd4d93fd89369f6ea0c0317f6788806eccede34",
      "version": 1,
      "is_coinbase": true,
      "inputs": [...],
      "outputs": [...],
      "status": "Confirmed",
      "block_height": 1,
      "block_hash": "0x0000000074857748e2050bff75595a287e1e7b507d2a4dcfabc9db4ee70ef552"
    }
  ]
}
```

---

## 3. `GET /api/v1/blocks/{height_or_hash}`
Mengambil blok spesifik berdasarkan tinggi indeks rantai (integer) atau hash heksadesimal 32-byte berawalan `0x`.

### Contoh Permintaan:
```bash
curl -s http://127.0.0.1:8332/api/v1/blocks/0
curl -s http://127.0.0.1:8332/api/v1/blocks/0x4033f099ae89051a629c871e9af28a215898ff345505ffdbbce65c27a29585c9
```

---

## 4. `GET /api/v1/passbook?address={address}`
Mengambil daftar UTXO aktif, saldo Quanta total, dan riwayat transaksi untuk alamat Bech32 yang diberikan.

### Contoh Permintaan:
```bash
curl -s "http://127.0.0.1:8332/api/v1/passbook?address=scy1nw7vhxmxyz2jlw89vz88tdv938692xk968uxn89787fa4w207s8sddvv3q"
```

---

## 5. `POST /api/v1/tx`
Mengirimkan transaksi yang telah ditandatangani dalam format byte hex untuk divalidasi ke dalam mempool dan disebarkan ke jaringan via Gossipsub.

- **Header Wajib**: `Content-Type: application/json`
- **Body JSON**:
  ```json
  {
    "raw_tx_hex": "0100000001000000..."
  }
  ```

---

## 6. Penanganan Galat & Rate Limiting
- **Format Galat**: JSON `{ "error": "<PESAN_GALAT>" }`.
- **Status Codes**:
  - `400 Bad Request`: Format JSON atau heksadesimal invalid.
  - `404 Not Found`: Blok atau transaksi tidak ditemukan pada rantai kanonikal.
  - `422 Unprocessable Entity`: Transaksi gagal validasi konsensus / double spend.
  - `429 Too Many Requests`: Melampaui batas laju permintaan Nginx rate limit (burst exhausted).
  - `502 Bad Gateway`: Node backend tidak dapat dihubungi.
