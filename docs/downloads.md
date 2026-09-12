# Downloads & Releases

Pusat unduhan biner resmi ekosistem Scytale Blockchain L1 untuk platform Windows, Linux, dan Android. Seluruh biner telah dikompilasi secara deterministik dan dilengkapi tanda tangan kriptografis serta checksum SHA-256.

---

## Scytale Passbook Suite (v0.1.0)

**Scytale Passbook** adalah dompet buku besar (ledger) dan penjelajah saldo eUTXO interaktif multi-platform.

| Platform / Paket | Format | Ukuran | Checksum SHA-256 | Unduhan |
| :--- | :---: | :---: | :--- | :--- |
| **Android APK** | APK | 98 MB | `57aa96ad55da42911bab3848d059669804bcb59d88c695d3530523647288f44e` | [scytale-passbook_0.1.0.apk](https://github.com/ratufatma/scytale-passbook/releases/download/v0.1.0/scytale-passbook_0.1.0.apk) |
| **Linux DEB** | Debian | 2.8 MB | `757cc4931fde15a90369fb1eecdce19f89052c8772de61eb62fa9211480b4b24` | [scytale-passbook_0.1.0_amd64.deb](https://github.com/ratufatma/scytale-passbook/releases/download/v0.1.0/scytale-passbook_0.1.0_amd64.deb) |
| **Linux AppImage** | Portable | 70 MB | `c9baa1e42a16eafdfbd841a4b89bd508040faf0e7f93a40354e39a367540360c` | [scytale-passbook_0.1.0_amd64.AppImage](https://github.com/ratufatma/scytale-passbook/releases/download/v0.1.0/scytale-passbook_0.1.0_amd64.AppImage) |
| **Windows Setup** | NSIS (.exe) | 1.83 MB | `c647f2427a2234f4f03e23c6858cad647586c0d80e729e54e7ac924be615eda9` | [Scytale Passbook_0.1.0_x64-setup.exe](https://github.com/ratufatma/scytale-passbook/releases/download/v0.1.0/Scytale.Passbook_0.1.0_x64-setup.exe) |
| **Windows MSI** | MSI (.msi) | 2.74 MB | `eda8ce20fdf252f4f2df9006a6020c05a3a0fa464dc5fc6a8353513f489e6582` | [Scytale Passbook_0.1.0_x64_en-US.msi](https://github.com/ratufatma/scytale-passbook/releases/download/v0.1.0/Scytale.Passbook_0.1.0_x64_en-US.msi) |

---

## Scytale Studio IDE (v0.1.0)

**Scytale Studio** adalah lingkungan pengembangan terintegrasi (IDE) khusus pengembang kontrak pintar, simulasi transaksi eUTXO, dan interaksi node Scytale.

| Platform / Paket | Format | Ukuran | Checksum SHA-256 | Unduhan |
| :--- | :---: | :---: | :--- | :--- |
| **Linux DEB** | Debian | 5.4 MB | `7c58440d55d287c138bf15ef38cc17c24087b227e9366611f4a25f82bb734e46` | [scytale-studio_0.1.0_amd64.deb](https://github.com/ratufatma/scytale/releases/download/v0.1.0/scytale-studio_0.1.0_amd64.deb) |
| **Linux AppImage** | Portable | 73 MB | `0b799676f647d1f4ad8d117757ad00787c157fcf0268d7c2acbe6cf3f6a80467` | [scytale-studio_0.1.0_amd64.AppImage](https://github.com/ratufatma/scytale/releases/download/v0.1.0/scytale-studio_0.1.0_amd64.AppImage) |
| **Windows Setup** | NSIS (.exe) | 3.43 MB | `990a7e810e3e348e1d850a6bd5f2406e7d82620e421468ca05d1f52cfebe3ac8` | [Scytale Studio_0.1.0_x64-setup.exe](https://github.com/ratufatma/scytale/releases/download/v0.1.0/Scytale.Studio_0.1.0_x64-setup.exe) |
| **Windows MSI** | MSI (.msi) | 4.86 MB | `11ad1c3f4526ace2a7c1113180dd37752f4148c699c9112de786a3c7c06d65c3` | [Scytale Studio_0.1.0_x64_en-US.msi](https://github.com/ratufatma/scytale/releases/download/v0.1.0/Scytale.Studio_0.1.0_x64_en-US.msi) |

---

## Verifikasi Checksum SHA-256

Verifikasi integritas berkas unduhan sebelum instalasi:

```bash
# Linux / macOS
sha256sum -c SHA256SUMS

# Windows (PowerShell)
Get-FileHash -Algorithm SHA256 <NamaBerkas>
```
