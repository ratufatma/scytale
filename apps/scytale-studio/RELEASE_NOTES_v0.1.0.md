# Scytale Client Suite v0.1.0 Release Notes

Kami mengumumkan rilis resmi **v0.1.0** untuk paket aplikasi klien ekosistem Scytale: **Scytale Passbook** (Desktop & Mobile) dan **Scytale Studio** (Desktop IDE). Rilis ini menandai ketersediaan biner produksi mandiri (*standalone binaries*) yang terhubung langsung ke jaringan blockchain Scytale L1 (Testnet).

---

## 1. Sorotan Rilis: Scytale Passbook v0.1.0

**Scytale Passbook** adalah dompet buku besar (ledger) dan penjelajah saldo eUTXO berbasis kanvas interaktif modern yang dirancang untuk platform desktop (Windows & Linux) dan mobile (Android).

### Fitur Utama:
* **Integrasi RPC Node Langsung (Live RPC)**: Terhubung langsung ke node Scytale lokal (`http://127.0.0.1:8332`) untuk kueri status rantai blok, tinggi kanonikal, saldo Quanta/SCY, dan daftar UTXO aktif.
* **Failover Otomatis**: Mendeteksi status node secara otomatis. Jika node lokal tidak aktif atau tidak dapat dijangkau, koneksi secara mulus dialihkan ke gateway publik Explorer (`https://explorer.myratu.com`).
* **Mesin Kode QR Terintegrasi (3 Varian SVG)**:
  1. *Transfer Inbound QR*: Menghasilkan kode QR alamat dompet lengkap untuk mempermudah penerimaan transfer dana.
  2. *Ledger Mutation Audit QR*: Menghasilkan kode QR bukti mutasi kriptografis per transaksi.
  3. *Raw Transaction / UTXO Inspect QR*: Menghasilkan kode QR payload outpoint dan detail transaksi.
* **Arsitektur Multi-Target**:
  * **Desktop (Windows & Linux)**: Dibangun di atas Tauri v2, Vite 6, Tailwind CSS v4, Motion, dan React 19 dalam format installer Windows (`.exe` NSIS & `.msi` WiX), Linux Debian (`.deb`), dan portabel `.AppImage`.
  * **Mobile (Android)**: Dibangun menggunakan React Native 0.86 dan Expo SDK 57, ditenagai engine performa tinggi Hermes (`enableHermes: true`) serta arsitektur baru React Native.

---

## 2. Sorotan Rilis: Scytale Studio v0.1.0

**Scytale Studio** adalah lingkungan pengembangan terintegrasi (IDE) khusus pengembang kontrak pintar, simulasi transaksi eUTXO, dan interaksi node Scytale.

### Fitur Utama:
* **WebAssembly (Wasm) Consensus Engine**: Mengintegrasikan crate konsensus Rust inti (`scytale-core`) langsung ke dalam antarmuka desktop melalui Wasm untuk validasi transaksi offline.
* **Virtual Pseudo-Terminal (PTY)**: Terminal interaktif bawaan untuk menjalankan perintah pengujian, kompilasi skrip, dan inspeksi blockchain tanpa dependensi shell eksternal.
* **Smart Contract & Script Playground**: Editor kode modern untuk menulis dan memverifikasi skrip penguncian/pembuka P2PKH dan kontrak kondisi lanjutan.
* **Paket Desktop Mandiri**: Didistribusikan dalam bentuk installer Windows (`.exe` NSIS & `.msi` WiX), paket Debian (`.deb`), dan portable Linux executable (`.AppImage`).

---

## 3. Verifikasi Integritas & Checksum SHA-256

Semua biner resmi pada direktori `dist-clients/` telah diverifikasi dan memiliki checksum SHA-256 berikut:

| Berkas Biner | Platform / Arsitektur | Format | Ukuran | Checksum SHA-256 |
| :--- | :--- | :---: | :---: | :--- |
| **`scytale-passbook_0.1.0.apk`** | Android (Universal / ARM64 / x86) | Standalone APK | 98 MB | `57aa96ad55da42911bab3848d059669804bcb59d88c695d3530523647288f44e` |
| **`scytale-passbook_0.1.0_amd64.deb`** | Linux x86_64 (Debian / Ubuntu) | Debian Package | 2.8 MB | `757cc4931fde15a90369fb1eecdce19f89052c8772de61eb62fa9211480b4b24` |
| **`scytale-passbook_0.1.0_amd64.AppImage`** | Linux x86_64 (Distro-agnostic) | Portable AppImage | 70 MB | `c9baa1e42a16eafdfbd841a4b89bd508040faf0e7f93a40354e39a367540360c` |
| **`Scytale Passbook_0.1.0_x64-setup.exe`** | Windows x64 (Windows 10 / 11) | NSIS Installer | 1.83 MB | `c647f2427a2234f4f03e23c6858cad647586c0d80e729e54e7ac924be615eda9` |
| **`Scytale Passbook_0.1.0_x64_en-US.msi`** | Windows x64 (Windows 10 / 11) | Windows Installer (MSI) | 2.74 MB | `eda8ce20fdf252f4f2df9006a6020c05a3a0fa464dc5fc6a8353513f489e6582` |
| **`scytale-studio_0.1.0_amd64.deb`** | Linux x86_64 (Debian / Ubuntu) | Debian Package | 5.4 MB | `7c58440d55d287c138bf15ef38cc17c24087b227e9366611f4a25f82bb734e46` |
| **`scytale-studio_0.1.0_amd64.AppImage`** | Linux x86_64 (Distro-agnostic) | Portable AppImage | 73 MB | `0b799676f647d1f4ad8d117757ad00787c157fcf0268d7c2acbe6cf3f6a80467` |
| **`Scytale Studio_0.1.0_x64-setup.exe`** | Windows x64 (Windows 10 / 11) | NSIS Installer | 3.43 MB | `d39841acd979c37de2308708de8f1b03623c5a4dec3fbb2a160d4fca3e9e1b3c` |
| **`Scytale Studio_0.1.0_x64_en-US.msi`** | Windows x64 (Windows 10 / 11) | Windows Installer (MSI) | 4.86 MB | `5f5d5b63857c6f31e2e0667f8d10d02df01c1a752dcc74c8ec480a906fbfe5ce` |

Verifikasi mandiri checksum sebelum instalasi:
```bash
# Linux / macOS
sha256sum -c SHA256SUMS

# Windows PowerShell
Get-FileHash -Algorithm SHA256 (Get-ChildItem -Include "*.exe","*.msi")
```

---

## 4. Panduan Instalasi Cepat

### A. Windows Desktop (Windows 10 / 11 64-bit)

#### 1. Menggunakan NSIS Setup (.exe):
- Klik dua kali berkas instalasi:
  - `Scytale Passbook_0.1.0_x64-setup.exe` untuk Scytale Passbook.
  - `Scytale Studio_0.1.0_x64-setup.exe` untuk Scytale Studio.
- Ikuti petunjuk wizard instalasi di layar hingga selesai.

#### 2. Menggunakan Windows Installer (.msi):
- Klik dua kali berkas `.msi` atau jalankan via Command Prompt / PowerShell:
```cmd
# Instal Scytale Passbook
msiexec /i "Scytale Passbook_0.1.0_x64_en-US.msi"

# Instal Scytale Studio
msiexec /i "Scytale Studio_0.1.0_x64_en-US.msi"

# Opsi instalasi hening (silent installation):
msiexec /i "Scytale Passbook_0.1.0_x64_en-US.msi" /qn
```

---

### B. Linux Desktop (Debian / Ubuntu / Mint / Pop!_OS)

Instal paket `.deb` menggunakan `dpkg` atau `apt`:

```bash
# Scytale Passbook
sudo apt install ./scytale-passbook_0.1.0_amd64.deb

# Scytale Studio
sudo apt install ./scytale-studio_0.1.0_amd64.deb
```

---

### C. Linux Desktop (AppImage - Seluruh Distribusi)

Jalankan langsung file `.AppImage` setelah memberikan izin eksekusi:

```bash
# Berikan izin eksekusi
chmod +x scytale-passbook_0.1.0_amd64.AppImage scytale-studio_0.1.0_amd64.AppImage

# Jalankan Scytale Passbook
./scytale-passbook_0.1.0_amd64.AppImage

# Jalankan Scytale Studio
./scytale-studio_0.1.0_amd64.AppImage
```

*(Catatan: Pastikan sistem Anda telah memiliki pustaka `fuse` / `libfuse2t64` terpasang)*

---

### D. Android Mobile (ADB / Direct Install)

Pasang biner APK ke perangkat Android fisik atau emulator:

```bash
# Pasang via Android Debug Bridge (ADB)
adb install -r scytale-passbook_0.1.0.apk
```

Atau salin berkas `scytale-passbook_0.1.0.apk` langsung ke penyimpanan perangkat dan pasang menggunakan aplikasi File Manager (izinkan *Install from Unknown Sources*).
