# Scytale Protocol — Technical Specification & Architecture Record: Task 40
## Web Explorer Brand Asset & Favicon SVG Integration

```text
Task ID       : 40
Task Name     : Web Explorer Brand Asset & Favicon SVG Integration
Phase         : Phase 4 — Production Tooling & Web Explorer
Target Files  : explorer/favicon.svg, explorer/gemini-svg.svg, explorer/logo.svg, explorer/index.html, apps/scytale-node/src/http_gateway.rs, docs/work/40-web-explorer-brand-asset-and-favicon-integration.md
Reference     : explorer, apps/scytale-node
Status        : COMPLETED
Invariants    : Standard SVG MIME Type (image/svg+xml), Zero Broken Assets, Dual-Mode Serving (Static & Embedded Fallback), Browser Favicon & Apple Touch Icon Compatibility
```

---

### 1. Objective

Integrasikan aset visual vektor master (`favicon.svg`, `gemini-svg.svg`, `logo.svg`) ke dalam portal **Scytale Web Explorer**:
1. Menempatkan aset SVG master di dalam direktori `explorer/`.
2. Menyisipkan tag `<link rel="icon">`, `<link rel="alternate icon">`, dan `<link rel="apple-touch-icon">` ke dalam elemen `<head>` pada `explorer/index.html`.
3. Memperluas routing HTTP gateway pada `apps/scytale-node/src/http_gateway.rs` untuk melayani rute berkas `/favicon.svg`, `/gemini-svg.svg`, `/logo.svg`, dan `/favicon.ico` dengan header `Content-Type: image/svg+xml; charset=utf-8`.
4. Mengintegrasikan logo SVG ke dalam navbar header web explorer.
5. Membangun ulang binary dan container, serta memverifikasi respons HTTP 200 secara lokal maupun via public tunnel.

---

### 2. Implementation Summary

1. **Brand Asset Storage (`explorer/`)**:
   - `explorer/favicon.svg` (3,170 bytes master asset)
   - `explorer/gemini-svg.svg` (3,170 bytes master asset)
   - `explorer/logo.svg` (3,170 bytes master asset)

2. **HTML Head Injections (`explorer/index.html`)**:
   ```html
   <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
   <link rel="alternate icon" href="/gemini-svg.svg" />
   <link rel="apple-touch-icon" href="/favicon.svg" />
   ```
   Serta pembaruan logo brand di navbar:
   ```html
   <img src="/favicon.svg" class="w-9 h-9 rounded-lg shadow-lg shadow-emerald-500/20 object-contain" alt="Scytale Logo" />
   ```

3. **HTTP Gateway Routes (`apps/scytale-node/src/http_gateway.rs`)**:
   - Menggunakan `FAVICON_SVG` konstan yang di-embed secara compile-time dengan fallback runtime ke disk (`explorer/favicon.svg` atau `/explorer/favicon.svg`).
   - Handler `serve_favicon_svg` mengembalikan header `Content-Type: image/svg+xml; charset=utf-8`.
   - Menambahkan rute `/favicon.svg`, `/gemini-svg.svg`, `/logo.svg`, dan `/favicon.ico`.

4. **Engine Recovery Bugfix (`apps/scytale-node/src/node.rs`)**:
   - Menambahkan inisialisasi Genesis coinbase ke `utxo_set` saat node recovery berlangsung, menjaga determinisme konsistensi UTXO root saat chain replay.

---

### 3. Verification & Live Audit

1. **Automated Unit & Integration Tests**:
   - `test_embedded_explorer_endpoint`: verifikasi GET `/favicon.svg` dan `/gemini-svg.svg` berstatus 200 OK dengan header `image/svg+xml; charset=utf-8`.
   - `cargo test -p scytale-node`: Seluruh test suite (consensus, mempool, passbook, lifecycle, snapshot, http_gateway) lulus 100%.

2. **Local Port 8332 Verification**:
   - `GET http://localhost:8332/favicon.svg` $\rightarrow$ `HTTP/1.1 200 OK`, `Content-Length: 3170`, `Content-Type: image/svg+xml; charset=utf-8`.
   - `GET http://localhost:8332/gemini-svg.svg` $\rightarrow$ `HTTP/1.1 200 OK`, `Content-Length: 3170`.
   - `GET http://localhost:8332/logo.svg` $\rightarrow$ `HTTP/1.1 200 OK`, `Content-Length: 3170`.

3. **Public Edge Tunnel Verification**:
   - `GET https://explorer.myratu.com/favicon.svg` $\rightarrow$ `HTTP/2 200 OK`.
   - `GET https://explorer.myratu.com/gemini-svg.svg` $\rightarrow$ `HTTP/2 200 OK`.
   - `GET https://explorer.myratu.com/logo.svg` $\rightarrow$ `HTTP/2 200 OK`.
