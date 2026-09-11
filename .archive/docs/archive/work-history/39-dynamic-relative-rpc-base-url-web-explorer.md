# Scytale Protocol — Technical Specification & Architecture Record: Task 39
## Dynamic Relative RPC Base URL & Responsive Web Explorer Fix

```text
Task ID       : 39
Task Name     : Dynamic Relative RPC Base URL & Responsive Web Explorer Fix
Phase         : Phase 4 — Production Tooling & Web Explorer
Target Files  : explorer/index.html, apps/scytale-node/src/http_gateway.rs, docs/work/39-dynamic-relative-rpc-base-url-web-explorer.md
Reference     : apps/scytale-node, explorer
Status        : IN PROGRESS
Invariants    : Zero Hardcoded Loopback in Production Web Client, Dynamic Origin Fallback, Cloudflare Tunnel Ingress Compatibility, Mobile & External Responsive Online Status
```

---

### 1. Root Cause Analysis

Ketika Web Explorer diakses dari perangkat eksternal (smartphone atau komputer jarak jauh) melalui Cloudflare Tunnel (`https://explorer.myratu.com` atau `https://noether-network.click`), frontend menampilkan status **OFFLINE**.

Penyebab:
1. Input field `#nodeUrl` pada `explorer/index.html` memiliki nilai default `value="http://127.0.0.1:8332"`.
2. Fungsi JavaScript `getBaseUrl()` membaca nilai tersebut secara harfiah, sehingga browser pada perangkat mobile mencoba melakukan kueri HTTP RPC ke `http://127.0.0.1:8332` di perangkat pengguna sendiri (bukan ke node host).
3. Permintaan gagal (*Connection Refused / Failed to Fetch*), menyebabkan antarmuka terjebak dalam status **OFFLINE**.

---

### 2. Architecture & Solution Design

1. **Dynamic Origin Resolution**:
   - `getBaseUrl()` secara otomatis mendeteksi `window.location.origin`. Jika aplikasi dilayani melalui `https://explorer.myratu.com`, origin tersebut langsung digunakan sebagai endpoint RPC.
   - Input `#nodeUrl` diisi secara dinamis dengan `window.location.origin` saat halaman dimuat, sambil tetap mempertahankan fleksibilitas jika pengguna ingin mengganti node URL secara manual.
   - Jika `#nodeUrl` dikosongkan, sistem secara otomatis menggunakan path relatif `/api/v1/...` (meminimalisir masalah CORS dan mixed-content HTTPS).

2. **Backend Serving Optimization**:
   - `apps/scytale-node/src/http_gateway.rs` melayani `explorer/index.html` baik secara kompilasi (*embedded fallback*) maupun pengecekan berkas lokal jika tersedia pada jalur kerja runtime.

3. **Verifikasi**:
   - Uji respon curl lokal `http://127.0.0.1:8332`.
   - Uji respon publik `https://explorer.myratu.com/api/v1/status` dan `https://noether-network.click/api/v1/status`.
   - Verifikasi status ONLINE pada perangkat mobile dan jaringan eksternal.
