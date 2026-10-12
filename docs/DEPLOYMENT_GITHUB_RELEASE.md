# Deployment & GitHub Release — RigPulse

Panduan praktis menerbitkan rilis RigPulse dan mengaktifkan pembaruan otomatis
dari repositori GitHub. Ditulis setelah rilis `v0.4.0` diterbitkan ke
`zyilzzz77/RigPulse`.

## Ringkas

- Kode sumber sudah dipush ke branch `main`.
- Rilis `v0.4.0` diterbitkan sebagai GitHub Release berisi 4 aset:
  - `RigPulse-0.4.0-x64.msi` — installer resmi (per-user, UpgradeCode tetap).
  - `RigPulse-0.4.0-x64-portable.zip` — build portabel tanpa installer.
  - `SHA256SUMS.txt` — hash verifikasi lokal.
  - `stable.json` — manifest pembaruan **ditandatangani** Ed25519.
- Aplikasi memeriksa `stable.json` secara otomatis (interval 24 jam, saat
  `auto_check_updates` aktif) dan menawarkan pembaruan melalui UI yang
  memerlukan konfirmasi eksplisit.

## Skema keamanan (fail-closed)

Pembaruan **tidak** mengandalkan kepercayaan pada GitHub semata. Alur:

1. `crates/rigpulse-updater/release-config.json` memuat _public_ key Ed25519,
   URL manifest, dan origin paket. File ini di-commit; private seed **tidak**.
2. `stable.json` berisi `payload_base64` (Release JSON) + `signature_base64`.
3. Verifier menolak jika tanda tangan tidak sah, versi pratinjau, kedaluwarsa,
   arsitektur bukan `x86_64-pc-windows-msvc`, ukuran/hash berubah, atau host
   paket di luar origin yang diizinkan.
4. Unduhan mematuhi redirect **hanya** ke `release-assets.githubusercontent.com`
   (CDN aset resmi GitHub); redirect ke host lain dihentikan.
5. Setelah unduh: verifikasi SHA-256, fsync, rename atomik, lalu validasi identitas
   MSI (ProductName=RigPulse, versi cocok, UpgradeCode
   `{E10F4D59-89E4-44BC-BB62-C6C8DE55357F}`, template x64).
6. Instalasi memanggil `msiexec.exe /i <paket> /norestart` dengan UI Windows
   Installer terlihat — tanpa silent elevation/UAC bypass.

## Menandatangani rilis (publisher)

Seed penandatanganan 32 byte disimpan **lokal** di `keys/release_signing_key.base64`
(gitignored, jangan di-commit). Untuk rilis berikutnya:

```powershell
$env:RIGPULSE_RELEASE_SIGNING_KEY_BASE64 = (Get-Content keys/release_signing_key.base64)
$ver = "0.5.0"   # sesuaikan
$tag = "v$ver"
$msi = "dist/RigPulse-$ver-x64.msi"
$size = (Get-Item $msi).Length
$sha  = (Get-FileHash $msi -Algorithm SHA256).Hash.ToLower()
$manifest = @{
  schema=1; product="RigPulse"; version=$ver
  published_at=(Get-Date -AsUTC -UFormat "+%Y-%m-%dT%H:%M:%SZ")
  expires_at=(Get-Date -AsUTC).AddDays(365).ToString("yyyy-MM-ddTHH:mm:ssZ")
  notes="RigPulse $ver"
  package=@{ url="https://github.com/zyilzzz77/RigPulse/releases/download/$tag/RigPulse-$ver-x64.msi"
             target="x86_64-pc-windows-msvc"; kind="msi"; size=[long]$size; sha256=$sha }
}
$manifest | ConvertTo-Json -Depth 5 | Set-Content -Encoding utf8 "dist/release.json"
cargo run -p rigpulse-updater --example sign_release --locked -- "dist/release.json" "dist/stable.json"
```

## Menerbitkan rilis

### Opsi A — Manual (GitHub UI)

1. `cargo build --release -p rigpulse --locked` lalu `./scripts/package.ps1 -WixBin <wix>`.
2. Di GitHub: **Releases → Draft a new release**, tag `vX.Y.Z` (harus cocok
   dengan `version` di `Cargo.toml` dan `package.url` di manifest).
3. Upload `RigPulse-X.Y.Z-x64.msi`, `-portable.zip`, `SHA256SUMS.txt`, `stable.json`.
4. Publish. Aplikasi versi lama akan mendeteksi & menawarkan pembaruan.

### Opsi B — Otomatis via CI (tag push)

`.github/workflows/ci.yml` memiliki job `release` yang berjalan saat push tag
`v*`. Ia mem-build, memaketkan, **menandatangani manifest** (butuh secret repo
`RIGPULSE_RELEASE_SIGNING_KEY_BASE64` = isi `keys/release_signing_key.base64`),
lalu mempublish release dengan `softprops/action-gh-release`.

Aktifkan sekali:
Settings → Secrets → Actions → New repository secret
`RIGPULSE_RELEASE_SIGNING_KEY_BASE64` = `<isi file keys/release_signing_key.base64>`

Lalu: `git tag v0.5.0 && git push origin v0.5.0`.

## Rotasi kunci

Jika private seed bocor: buat pasangan baru, perbarui `public_key_base64` di
`release-config.json`, commit, dan terbitkan rilis baru dengan manifest
ditandatangani key baru. Klien lama yang memakai public key lama tetap bisa
memverifikasi rilis lama hingga kedaluwarsa; rilis baru butuh build dengan
public key baru.

## Pembatasan

- Hanya kanal stabil (semver tanpa pre-release) yang ditawarkan.
- Origin paket harus `https://github.com`; aset GitHub me-redirect ke CDN resmi
  yang diizinkan. Host lain ditolak.
- Tanpa koneksi/TLS valid, pemeriksaan mengembalikan error secara jujur (tanpa
  panic) dan tidak memasang apa pun.
- Verifikasi end-to-end online (download sunggung + upgrade MSI per-user) belum
  diklaim di sandbox ini; alur kriptografi/size/hash/MSI sudah diuji lewat
  `verify_local` dan test `rigpulse-updater`.
