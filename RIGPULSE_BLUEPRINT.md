# RIGPULSE — PRODUCT & ENGINEERING BLUEPRINT

> **Dokumen utama untuk AI Coding Agent**  
> Project: **RigPulse**  
> Versi blueprint: **1.0**  
> Tanggal: **11 Oktober 2026**  
> Bahasa aplikasi: **Bahasa Indonesia (default)**, English (opsional)  
> Platform: **Windows 10/11 x64 desktop**  
> Stack: **Rust-only native GUI + Windows API + SQLite**  
> Status: **Spesifikasi implementasi; bukan klaim bahwa aplikasinya sudah dibangun**

---

## 0. INSTRUKSI WAJIB UNTUK AI AGENT

Kamu adalah software engineer senior Rust/Windows. Bangun aplikasi **RigPulse**, bukan website. Perlakukan dokumen ini sebagai **sumber kebutuhan produk**, tetapi tetap verifikasi dokumentasi API, ketersediaan library, dan kemampuan perangkat sebelum implementasi.

**Aturan pengerjaan:**

1. **Gunakan Rust untuk seluruh kode aplikasi dan antarmuka pengguna.** GUI menggunakan `egui` + `eframe`. Jangan menggunakan React, HTML/CSS, JavaScript, Electron, Tauri, atau WebView untuk GUI utama.
2. **Buat aplikasi Windows GUI yang benar-benar berjalan**, tidak berhenti pada mockup/screenshot. Pengguna tidak perlu memakai terminal atau CMD untuk menggunakan RigPulse. Developer boleh menggunakan Cargo, PowerShell, dan CI untuk build.
3. **Tampilan wajib dark navy**, sederhana untuk pemula, responsif terhadap ukuran jendela, tanpa emoji. Semua ikon menggunakan ikon vektor/outline konsisten.
4. **Tombol tindakan `Update Driver`, `Install Driver`, dan `Update All` menggunakan warna hijau**. Tombol hanya aktif apabila tindakannya benar-benar didukung, aman, dan dapat dijalankan. Tombol lain memakai warna netral/navy; tindakan destruktif merah.
5. **Utamakan data nyata**, jangan menampilkan angka dummy sebagai data hasil scan. Jika hardware/API tidak mendukung data, tampilkan **"Tidak tersedia"**, bukan `0°C`, `100% sehat`, atau teks lain yang menyesatkan.
6. **Tidak menginstal driver otomatis tanpa persetujuan pengguna.** Jangan mengunduh driver dari repositori tak terpercaya, jangan menonaktifkan signature enforcement, antivirus, atau keamanan Windows.
7. **Read-only by default** untuk monitoring dan scan. Fitur modifikasi sistem wajib dipisahkan dari fitur baca serta meminta izin/elevation hanya bila diperlukan.
8. **Tidak membuat driver kernel khusus hanya untuk membaca sensor** pada versi awal. Jangan memakai driver kernel unsigned atau bypass keamanan hardware.
9. **Offline-first:** monitoring lokal dan informasi sistem tetap berjalan tanpa internet. Pencarian/unduhan update driver memerlukan internet.
10. Gunakan **error handling**, logging lokal yang tidak membocorkan informasi sensitif, unit test, integration test, dan validasi di Windows nyata.
11. Kerjakan bertahap mengikuti fase pengembangan. **Jangan mengklaim fitur selesai sebelum berjalan dan lolos acceptance criteria.**
12. Jika ada kemampuan yang belum dapat dijamin (misalnya CPU temperature pada semua laptop), buat antarmuka abstraksi, tampilkan fallback yang jujur, dokumentasikan keterbatasannya.
13. Jangan menganggap "driver paling baru" sebagai string versi numerik terbesar. Hormati kebijakan Windows dan pemilihan paket driver berdasarkan kompatibilitas, signature, hardware ID, dan ranking paket.
14. Jangan gunakan hasil scan sintetis dalam build produksi. Mode pengujian menggunakan mock provider yang terpisah dan diberi label jelas.
15. Tulis README, instruksi build, daftar fitur yang sudah diimplementasikan, keterbatasan, dan langkah pengujian ketika menyelesaikan setiap fase.

### Keluaran yang diminta dari agent

- Source code Rust lengkap dalam workspace Cargo.
- Executable `RigPulse.exe` berjenis GUI desktop.
- Installer `.msi` untuk Windows x64 (dan `.exe` installer opsional).
- `README.md`, `docs/ARCHITECTURE.md`, `docs/LIMITATIONS.md`, `docs/TEST_PLAN.md`.
- Tes dan workflow build/release Windows.
- GUI yang seluruh navigasi dan aksi pentingnya dapat dipakai, tanpa placeholder palsu di rilis final.

---

## 1. PRODUCT VISION

RigPulse adalah **Windows Hardware Monitor, Driver Manager, Battery Health Monitor, dan System Diagnostics** dalam satu aplikasi lokal. Targetnya pemula yang ingin tahu kondisi laptop/PC tanpa harus membuka Device Manager, Task Manager, System Information, dan beberapa aplikasi monitoring berbeda.

### Tujuan

1. Menampilkan merek dan tipe laptop/PC, CPU, GPU, RAM, SSD/HDD, motherboard, BIOS, display, dan jaringan.
2. Menampilkan penggunaan hardware realtime dan suhu CPU/GPU **jika sensornya tersedia**.
3. Memeriksa driver yang terinstal, driver yang error/hilang, dan update driver yang ditawarkan melalui sumber resmi.
4. Mengunduh dan menginstal update driver dengan **persetujuan pengguna**, indikator progres, log hasil, dan petunjuk restart.
5. Memeriksa level baterai, kesehatan baterai berdasarkan kapasitas (jika tersedia), dan charging status.
6. Menjalankan pemeriksaan masalah dasar Windows/hardware serta menyajikan alasan temuan dan saran tindakan.
7. Menyimpan riwayat scan/update dan mengekspor laporan.
8. Mendukung berbagai merek laptop/PC Windows, tidak terkunci ke Lenovo.

### Non-goals versi 1.0

- Tidak membuat antivirus, overclocking, fan controller, flashing BIOS/UEFI, atau automatic BIOS updates.
- Tidak menjamin mengetahui semua sensor/kerusakan hardware.
- Tidak menyediakan katalog driver proprietary buatan sendiri atau melakukan scraping massal driver OEM.
- Tidak membuat cloud account, login/register, telemetry tersembunyi, atau backend publik.
- Tidak menginstal semua driver secara diam-diam saat startup.
- Tidak memakai informasi WMI yang hanya menyerupai sensor suhu CPU sebagai pembacaan inti CPU tanpa verifikasi.

---

## 2. TARGET PLATFORM & KOMPATIBILITAS

| Parameter | Requirement |
|---|---|
| Sistem operasi | Windows 10 x64 dan Windows 11 x64 |
| Toolchain | Stable Rust, target `x86_64-pc-windows-msvc` |
| Perangkat | Laptop dan desktop dari berbagai brand |
| Vendor CPU | Intel dan AMD; vendor lain hanya jika teruji |
| GPU | NVIDIA, AMD, Intel; tingkat informasi berbeda per provider |
| Startup | Bisa dijalankan tanpa admin untuk fitur baca umum |
| Internet | Tidak dibutuhkan untuk scan lokal; dibutuhkan untuk cek/download update |
| UI scaling | DPI 100–200%, jendela resizable |
| Ukuran minimum yang ditargetkan | Sekitar 1024 × 650 px; gunakan scrolling pada konten panjang |
| Bahasa | Indonesia default; siapkan i18n untuk Inggris |
| Unsupported data | Tampilkan `Tidak tersedia` disertai alasan singkat |

**Compatibility tiers:**

- **Basic:** info OS/hardware, penggunaan CPU/RAM, perangkat Windows, driver terpasang.
- **Enhanced:** suhu, GPU telemetry, SMART/storage health, dan detail baterai bila tersedia.
- **Limited:** bagian yang tak tersedia dinonaktifkan/ditandai, **fitur lainnya tetap bekerja**.

Jangan hardcode berdasarkan nama model. Gunakan fitur deteksi kemampuan (**capability detection**). Misalnya, PC tanpa baterai wajib menampilkan "Tidak ada baterai terdeteksi"; bukan error fatal.

---

## 3. RUST TECH STACK

| Layer | Teknologi / pendekatan |
|---|---|
| Language | Rust stable (MSVC Windows target) |
| Native GUI | `egui` + `eframe` |
| Realtime charts | `egui_plot`, pin versi kompatibel dengan `egui` |
| Windows platform bindings | `windows` (`windows-rs`) dengan fitur API yang diperlukan |
| WMI / SMBIOS | WMI Windows via `wmi` atau binding Windows API |
| CPU, RAM, disk/network basics | `sysinfo` + native Windows API jika perlu |
| NVIDIA telemetry | NVML melalui library FFI Rust yang terpelihara, hanya jika NVML tersedia |
| AMD / Intel telemetry | Provider khusus yang diuji; fallback unsupported |
| Device enumeration | SetupAPI dan Windows Configuration Manager API |
| Driver updates | Windows Update Agent (WUA) COM API; sesuai kebijakan OS |
| Battery | Windows Battery API dan optional WMI dengan validasi |
| Windows diagnostics | Windows Event Log, device problem codes, storage API |
| Database local | SQLite via `rusqlite` |
| Serialization | `serde`, `serde_json` |
| Errors | `thiserror`; `anyhow` untuk batas aplikasi |
| Observability | `tracing`, `tracing-subscriber` atau ekuivalen |
| Async / task runner | Worker thread/channel; Tokio hanya jika benar-benar dibutuhkan |
| Reports | PDF via library Rust teruji, dan JSON via serde |
| Windows MSI | WiX Toolset dan `cargo-wix` (sesuaikan versi WiX yang kompatibel) |
| CI | GitHub Actions Windows runner |

**Dependency policy:** verifikasi crate API versi terkini sebelum coding, pin compatible versions di `Cargo.lock`, jangan berasumsi API stabil lintas major/minor. Hindari dependency berlisensi tidak cocok untuk distribusi. Native DLL vendor opsional tidak sama dengan menulis backend dalam bahasa lain: kode aplikasi tetap Rust; integrasi Windows/vendor dilakukan via FFI.

### Performance targets (sasaran, bukan janji)

- Startup tidak membekukan UI ketika inventory/scan berjalan.
- Pengambilan sampel sensor tidak blocking thread GUI.
- Refresh visual 1–2 detik untuk data aktif; sensor yang lebih lambat di-refresh lebih jarang.
- Penggunaan CPU saat idle/minimized diusahakan rendah, diuji di berbagai perangkat.
- History monitoring menggunakan ring buffer terikat untuk menghindari penggunaan memori tak terbatas.

---

## 4. UI/UX DESIGN SYSTEM

### 4.1 Warna (gunakan token konsisten)

```text
APP_BACKGROUND       #0B1220    dark navy
SIDEBAR_BACKGROUND   #101B2E
PANEL_BACKGROUND     #16243A
PANEL_HOVER          #1D304A
BORDER               #27384F
TEXT_PRIMARY         #F1F5F9
TEXT_SECONDARY       #94A3B8
GREEN_ACTION         #22C55E    update/install driver
GREEN_ACTION_HOVER   #16A34A
GREEN_ACTION_TEXT    #052E16
BLUE_INFO            #60A5FA
WARNING              #F59E0B
DANGER               #EF4444
DISABLED_BG          #334155
DISABLED_TEXT        #94A3B8
```

### 4.2 UX rules

- **Semua halaman GUI native**, satu window utama dan modal untuk detail/konfirmasi.
- Sidebar kiri selalu jelas; label menu utuh pada layar cukup besar.
- Bahasa mudah dimengerti; istilah teknis boleh ditampilkan sebagai subteks/tooltip.
- Ikon outline konsisten (SVG/icon font/vector; jangan emoji).
- Font Segoe UI atau font kompatibel Windows, ukuran konten 14–16 px (sesuaikan DPI).
- Border radius sekitar 10–12 px; spacing teratur 8/12/16/24 px.
- Status warna dipakai seperlunya. **Warna hijau CTA diutamakan untuk tombol install/update driver**, bukan seluruh komponen.
- Fokus keyboard, tab navigation, tooltip, state hover, loading, error, empty state wajib jelas.
- UI tidak pernah menampilkan scan palsu atau data contoh saat mode produksi.
- Semua daftar perangkat harus dapat dicari, difilter, dan discroll.
- Window resizable, sidebar compact bila ruang sempit. Jangan membuat horizontal scroll global.
- Jangan membuka CMD/PowerShell sebagai UI pengguna; interaksi sistem hanya lewat GUI.
- Jangan menampilkan serial number secara terbuka dalam screenshot/report kecuali pengguna memilih opsi penyertaan.

### 4.3 Layout dasar

```text
┌───────────────────────────────────────────────────────────────────────┐
│ RigPulse                                                _   □    X │
├────────────────┬──────────────────────────────────────────────────────┤
│                │ DASHBOARD                                    Refresh│
│  Dashboard     │ Lenovo LOQ / Windows 11 / Device Status              │
│  Hardware      │                                                      │
│  Monitoring    │ ┌────────────┐ ┌────────────┐ ┌────────────┐         │
│  Drivers       │ │ CPU usage  │ │ GPU / Temp │ │ Battery    │         │
│  Battery       │ └────────────┘ └────────────┘ └────────────┘         │
│  Diagnostics   │                                                      │
│  Reports       │ ┌────────────────────┐ ┌───────────────────────┐    │
│  Settings      │ │ Live Graph         │ │ Driver / System Status│    │
│                │ │                    │ │ [Check Driver Updates]│    │
│                │ └────────────────────┘ └───────────────────────┘    │
└────────────────┴──────────────────────────────────────────────────────┘
```

**Catatan:** diagram adalah skema tata letak, bukan permintaan meniru angka atau menampilkan kartu dummy.

### 4.4 Sidebar pages

1. Dashboard
2. Hardware
3. Monitoring
4. Drivers
5. Battery
6. Diagnostics
7. Reports
8. Settings

Sidebar aktif menunjukkan indikator outline/selection. Gunakan judul halaman + satu kalimat penjelas + area konten. Pertahankan navigasi saat background scan bekerja.

---

## 5. FUNCTIONAL REQUIREMENTS / FITUR

### F01 — Dashboard Overview (P0)

**Data:** merek/model laptop, OS, CPU, GPU, installed RAM, storage total, CPU usage, RAM usage, suhu yang valid jika tersedia, battery state, jumlah device error, status pencarian update driver terakhir.

**Quick actions:** `Scan System`, `Check Drivers`, `Hardware Details`, `Refresh`.

**Acceptance:**
- Dashboard pertama kali terbuka tanpa menunggu scan panjang.
- Semua kartu memperbarui status saat data siap.
- Tiap kartu menunjukkan `Loading`, `Available`, `Unavailable`, atau `Error` dengan alasan.
- Jika tidak ada baterai, aplikasi tetap stabil.
- Jika tidak ada internet, dashboard tetap dapat diakses.

### F02 — Hardware Inventory (P0)

**Kelompok detail:**

- **Device:** manufacturer, model, system type, chassis type jika ada.
- **CPU:** vendor/model, core, thread, clock (jelas nominal/aktual), usage.
- **GPU:** adapter vendor/model, driver version, VRAM jika andal.
- **RAM:** physical total/available, memory module details bila dapat dibaca.
- **Motherboard/BIOS:** board manufacturer, board product, BIOS vendor/version/date, UEFI status jika didukung.
- **Storage:** model, capacity, bus/interface, free space, disk type.
- **Display:** resolution, refresh rate, adapters.
- **Network:** wireless/Ethernet/Bluetooth devices, driver info.
- **Audio/peripherals:** device names, status.
- **Windows:** edition, build, version, architecture, uptime.

**UI:** navigasi kategori, tabel dua kolom `Properti | Nilai`, tombol `Copy Specifications`, opsi `Include Sensitive Identifiers` (default OFF).

**Acceptance:** tidak menampilkan hasil tebakan; field yang belum tersedia ditandai jelas; data tersalin ke clipboard dengan format rapi.

### F03 — Live Monitoring (P0/P1)

**Metric kandidat:**

| Metric | Sumber | Refresh target | Fallback |
|---|---|---|---|
| CPU usage | `sysinfo` / Windows perf | 1 detik | Tidak tersedia |
| Memory usage | `sysinfo` | 1 detik | Tidak tersedia |
| Disk utilization / activity | Windows counters | 2 detik | Tidak tersedia |
| GPU utilization | Vendor provider | 1–2 detik | Tidak tersedia |
| CPU temperature | Verified hardware sensor provider | ~2 detik | Tidak tersedia |
| GPU temperature | Verified vendor provider | ~2 detik | Tidak tersedia |
| Fan RPM | Compatible hardware/vendor provider | ~3 detik | Tidak tersedia |
| Battery level/status | Windows Battery API | ~5 detik | Tidak ada baterai / tidak tersedia |

**GUI:** card untuk metrik; graph line series (5/15/60 menit), min/avg/max jika valid, pilihan refresh interval (Normal/Hemat Daya), pause graph, status sumber sensor.

**Implementasi penting:**
- Sensor API Windows generik tidak menjamin suhu CPU/GPU. Jangan mencampur `ACPI ThermalZone` dengan "CPU Package Temp" tanpa bukti.
- `sysinfo` digunakan untuk penggunaan CPU/RAM; suhu Windows mungkin tidak tersedia.
- **Provider interface**: setiap metric memuat satuan, vendor, label, timestamp, validity, dan capability.
- Bedakan `not_supported`, `permission_denied`, `driver_missing`, `temporarily_unavailable`, `stale`.
- Tampilkan timestamp terakhir. Bila pembacaan kedaluwarsa, beri label `Data tidak diperbarui`.
- Hindari polling agresif dan pastikan GUI tetap responsif.
- Peringatan suhu bersifat indikatif dan tidak boleh memakai satu batas "aman" untuk seluruh tipe CPU/GPU tanpa sumber/konfigurasi.

### F04 — Driver Manager (P0 read / P1 write)

**F04.1 — Driver inventory:**
- Enumerasi device nodes melalui SetupAPI/Configuration Manager.
- Baca friendly name, class, manufacturer, PnP instance ID (jangan dipublikasikan otomatis), hardware IDs, driver provider, driver date/version, device status/problem code jika tersedia.
- Grup: Display, Network, Audio, Chipset/System, Bluetooth, Storage, Peripherals, Unknown.
- Status: `Working`, `Issue detected`, `Missing driver`, `Disabled`, `Unknown`.
- Pisahkan status **"Driver bermasalah"** dari **"Ada pembaruan"**. Perangkat sehat belum tentu mempunyai update; perangkat dengan error belum tentu bisa diperbaiki dengan update.

**F04.2 — Check updates:**
- Sumber pertama: Windows Update Agent (WUA) dan kebijakan Windows Update setempat.
- Cari update driver yang *ditawarkan* untuk komputer tersebut, jangan mengganti dengan versi numerik tertinggi dari internet.
- Tampilkan judul, provider/source, status download, kategori, tanggal/versi target jika tersedia, kebutuhan restart bila diketahui.
- Untuk driver OEM yang tidak tersedia melalui WUA: sediakan `Open Official Support` atau `Open Windows Update`, bukan tombol instalasi palsu.
- Jika Windows Update diblokir, tidak bisa diakses, atau policy organisasi membatasi: tampilkan alasan tanpa mengakali kebijakan.

**F04.3 — Install driver / update:**
- Pengguna memilih 1 atau beberapa paket yang tersedia dan telah lulus validasi.
- Tampilkan dialog ringkasan: `Perangkat`, `Sumber`, `Pembaruan`, `Risiko/konsekuensi`, `Restart mungkin diperlukan`.
- Minta persetujuan eksplisit, lalu UAC hanya bila dibutuhkan.
- Jalankan proses download/install melalui mekanisme Windows yang didukung; jangan mengeksekusi file tak terverifikasi.
- Tampilkan progress berdasarkan status API yang benar (bila tersedia); jika tak ada persentase tepat, gunakan indeterminate loading bukan angka palsu.
- Setelah operasi: verifikasi ulang installed driver/device status, tampilkan success/partial/failure, restart required bila terdeteksi.
- Tuliskan hasil dan timestamp ke riwayat.
- `Update All` hanya mencakup paket driver kompatibel dari sumber tervalidasi yang cocok dengan mode instalasi aman. BIOS/firmware **dikecualikan** dari Update All v1.
- Sediakan pilihan rollback **hanya jika benar-benar didukung oleh Windows**; jangan menjamin rollback otomatis.

**State machine driver updates:**

```text
Idle
  -> Scanning
  -> ResultsAvailable | NoUpdates | ScanFailed
ResultsAvailable
  -> UserSelected
  -> AwaitingConfirmation
  -> AwaitingElevation (if necessary)
  -> Downloading
  -> Installing
  -> Verifying
  -> Completed | CompletedRestartRequired | Failed | PartiallyCompleted
```

**Invariant:** tidak pernah masuk `Downloading/Installing` tanpa user consent; tidak ada aksi jika sumber tidak dipercaya; error tetap bisa dipulihkan tanpa aplikasi crash.

**Driver UI table:**

| Kolom | Isi |
|---|---|
| Device | Nama perangkat |
| Installed Version | Versi sekarang |
| Available Update | Versi/keterangan dari WUA jika ada |
| Status | Healthy / Issue / Update Available / Unknown |
| Action | Details / Find / Update / Install |

**Green buttons:**
- `Update Driver` / `Install Driver` background `#22C55E`, text `#052E16`, hover `#16A34A`.
- `Update All` green **hanya aktif jika ada paket yang eligible**.
- Jika tidak ada pembaruan, disable/hidden action dan tampilkan penjelasan.

**Security:**
- Jangan membuat scanner "driver terbaru" dengan scraping situs sembarang.
- Jangan memasang INF berdasarkan kecocokan string nama semata.
- Hormati ranking driver Windows, signature, device match, dan policy.
- Tidak mengubah Windows Update configuration atau menonaktifkan proteksi.
- Create Restore Point dapat ditawarkan saat tersedia/diizinkan, namun sifatnya best effort. Gagal membuat restore point tidak boleh diam-diam diabaikan.
- Bila write operations belum diimplementasi, UI harus jujur menampilkan `Pengecekan tersedia; instalasi belum didukung` (bukan simulasi sukses).

### F05 — Battery Health (P0/P1)

**Data:** current charge %, charging/AC/discharging state, design capacity, full charge capacity, cycle count, estimated battery health, waktu tersisa jika andal, wear level.

**Formula apabila kapasitas terverifikasi dan satuannya sebanding:**

```text
battery_health_percent = (full_charge_capacity / design_capacity) * 100
battery_wear_percent   = 100 - battery_health_percent
```

**Catatan:** hanya hitung bila `design_capacity > 0`, kedua nilai valid dan satuan cocok. Capacity dari driver tertentu mungkin relative/unknown. CycleCount = 0 dapat berarti tidak didukung, bukan pasti 0 siklus. Jangan memaksa output 100%. Jika tidak dapat dihitung, tampilkan `Tidak tersedia`.

**UI:** persentase charge, perkiraan kesehatan baterai, charging indicator, capacity comparison, history changes. PC tanpa baterai menampilkan empty state yang tepat.

### F06 — System Diagnostics (P1)

**Pemeriksaan aman dan read-only:**

1. Device Manager problem codes (misalnya Code 10, Code 28, Code 43 jika ditemukan).
2. Masalah driver yang dapat diverifikasi (disabled, missing, failed to start).
3. Event Log relevan untuk system/hardware errors, rentang waktu terpilih (misalnya 7 hari).
4. Storage health/SMART/NVMe health jika didukung, dengan confidence/source.
5. Battery health jika valid.
6. CPU/RAM/disk utilization anomaly berdasarkan jendela waktu sampel, bukan satu snapshot.
7. Temperature alerts jika sensor benar-benar tersedia.
8. Informasi restart pending jika API yang sesuai mendukung.

**Format setiap temuan:**

```text
id
category
severity: info | warning | critical
status: detected | not_detected | unavailable
headline
plain_language_explanation
evidence: API/source + exact error/code/value
recommended_action
can_auto_fix: false by default
timestamp
```

**Status laporan scan keseluruhan:** `Tidak ditemukan masalah pada pemeriksaan yang berhasil`, `Perlu perhatian`, `Masalah terdeteksi`, atau `Pemeriksaan belum lengkap`. Hindari klaim "Laptop 100% sehat".

**Penting:** `SFC /scannow`, `DISM`, memory test, dan fix network bukan bagian scan pasif; bila kelak disediakan, lakukan sebagai aksi eksplisit dengan penjelasan/izin, dan jangan otomatis mengeksekusi perintah sistem.

### F07 — Reports & History (P1)

- History per scan: tanggal, tipe scan, durasi, status, ringkasan, temuan.
- History driver updates: paket, sumber, waktu, status, pesan kegagalan, restart.
- Optional monitoring snapshots, sampling dibatasi sehingga database tidak membengkak.
- Export JSON dan PDF.
- Default report **menyembunyikan serial number, PnP ID, dan identitas jaringan sensitif**; pengguna dapat memilih menyertakannya.
- Tombol `Open Report Folder`, `Delete History`, dan export.
- Export sukses hanya jika file benar-benar dibuat.

### F08 — Settings (P1)

- Language: Indonesia / English jika terimplementasi.
- Monitoring refresh: Normal / Battery Saver.
- Start with Windows: **OFF** default; jika diterapkan, jelas dan bisa dibatalkan.
- Minimize to tray: opsional; target fase akhir.
- Show temperature warnings: ON/OFF, tergantung ketersediaan sensor.
- Save monitoring history: OFF atau minimal secara default (privasi/ukuran).
- Export/report privacy settings.
- App version, licenses, source information, update policy.
- Reset preferences dan clear logs/history.

---

## 6. DESAIN ARSITEKTUR

Gunakan Cargo workspace. Pisahkan GUI, application state, integrasi Windows, monitoring, driver operations, diagnostics, penyimpanan, laporan.

```text
rigpulse/
├── Cargo.toml                     # workspace
├── Cargo.lock
├── README.md
├── LICENSE
├── rust-toolchain.toml            # pin toolchain kompatibel
├── .gitignore
├── .github/
│   └── workflows/
│       ├── ci.yml
│       └── release.yml
├── apps/
│   └── rigpulse/
│       ├── Cargo.toml
│       ├── build.rs                # version/icon/manifests as needed
│       └── src/
│           ├── main.rs
│           ├── app.rs
│           ├── theme.rs
│           ├── navigation.rs
│           ├── ui_state.rs
│           ├── pages/
│           │   ├── dashboard.rs
│           │   ├── hardware.rs
│           │   ├── monitoring.rs
│           │   ├── drivers.rs
│           │   ├── battery.rs
│           │   ├── diagnostics.rs
│           │   ├── reports.rs
│           │   └── settings.rs
│           └── widgets/
│               ├── sidebar.rs
│               ├── metric_card.rs
│               ├── hardware_table.rs
│               ├── line_graph.rs
│               ├── driver_row.rs
│               ├── status_tag.rs
│               ├── progress.rs
│               └── confirmation_dialog.rs
├── crates/
│   ├── rigpulse-core/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── model.rs
│   │       ├── commands.rs
│   │       ├── events.rs
│   │       ├── capabilities.rs
│   │       ├── state.rs
│   │       └── errors.rs
│   ├── rigpulse-windows/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── inventory.rs
│   │       ├── wmi.rs
│   │       ├── devices.rs
│   │       ├── battery.rs
│   │       ├── event_log.rs
│   │       ├── storage.rs
│   │       └── permissions.rs
│   ├── rigpulse-monitor/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── provider.rs
│   │       ├── cpu.rs
│   │       ├── memory.rs
│   │       ├── gpu.rs
│   │       ├── temperature.rs
│   │       ├── disk.rs
│   │       └── buffer.rs
│   ├── rigpulse-drivers/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── scanner.rs
│   │       ├── update_source.rs
│   │       ├── wua.rs
│   │       ├── install.rs
│   │       ├── verify.rs
│   │       └── history.rs
│   ├── rigpulse-diagnostics/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── checks.rs
│   │       ├── findings.rs
│   │       └── engine.rs
│   ├── rigpulse-database/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── migrations.rs
│   │       ├── settings.rs
│   │       └── repository.rs
│   └── rigpulse-reports/
│       └── src/
│           ├── lib.rs
│           ├── json.rs
│           └── pdf.rs
├── assets/
│   ├── icons/
│   └── fonts/                     # only distributable, licensed fonts if needed
├── migrations/
│   └── 0001_initial.sql
├── tests/
│   ├── mock_hardware.rs
│   ├── driver_state_machine.rs
│   ├── battery_calculation.rs
│   └── report_privacy.rs
├── docs/
│   ├── ARCHITECTURE.md
│   ├── TEST_PLAN.md
│   ├── LIMITATIONS.md
│   └── RELEASE.md
└── wix/
    └── main.wxs
```

Struktur boleh disesuaikan apabila ada alasan teknis, tetapi batas tanggung jawab modul wajib dipertahankan.

### 6.1 Data flow

```text
Windows APIs / WMI / Vendor provider
                |
                v
Hardware and Driver Service Workers
                |
                v
Normalization + Capability / Error Model
                |
                v
Event Channel + Application State
           /               \
          v                 v
       egui GUI        SQLite History
                              |
                              v
                         PDF / JSON
```

### 6.2 Threading model

- **UI thread**: hanya render, navigasi, menerima input dan state terbaru.
- **Telemetry worker**: polling interval berdasarkan jenis sensor; cancellation saat app exit.
- **Inventory/diagnostic worker**: operasi berat yang dapat memakan waktu; channel hasil + progress.
- **Windows Update worker**: COM init/uninit pada thread yang menggunakan WUA, sesuai model threading API. Jangan memindahkan COM interface pointer secara sembarang antar-thread.
- **Database/report worker**: operasi write/export di luar UI thread.
- Semua operasi yang bisa gagal harus mengembalikan `Result` atau error domain yang jelas.
- Semua write operations dipicu dari user action setelah validasi.

### 6.3 Domain models (contoh kontrak, bukan kode final)

```rust
pub enum DataAvailability {
    Available,
    Unsupported,
    PermissionDenied,
    NotDetected,
    TemporarilyUnavailable,
    Error,
    Stale,
}

pub struct Measurement<T> {
    pub value: Option<T>,
    pub unit: String,
    pub label: String,
    pub source: String,
    pub collected_at_utc: String,
    pub availability: DataAvailability,
}

pub struct DeviceDriverInfo {
    pub instance_id: String,         // sensitive in reports
    pub friendly_name: String,
    pub manufacturer: Option<String>,
    pub provider: Option<String>,
    pub version: Option<String>,
    pub status: DeviceStatus,
    pub problem_code: Option<u32>,
}

pub enum DriverUpdateState {
    Idle,
    Scanning,
    ResultsAvailable,
    Downloading,
    Installing,
    Verifying,
    Completed,
    RestartRequired,
    Failed(String),
}
```

Semua value dari API OS harus divalidasi (unit, validity, sentinel values, timestamp). Simpan waktu dalam UTC dan render sesuai zona waktu lokal.

### 6.4 Provider interface

Rancang trait / adapter yang mengizinkan banyak implementasi:

```text
HardwareInfoProvider
BatteryInfoProvider
SensorProvider
DeviceEnumerationProvider
DriverUpdateProvider
DiagnosticCheck
ReportExporter
```

Ada `Mock*Provider` hanya untuk unit test, screenshot development, dan CI; tidak aktif secara diam-diam di rilis produksi. Jika provider A gagal, kegagalan itu tidak boleh menonaktifkan keseluruhan halaman.

---

## 7. DATABASE LOKAL

**Penyimpanan default:** `%LOCALAPPDATA%\RigPulse\` (dibuat bila belum ada, akses per-user).

```text
%LOCALAPPDATA%\RigPulse\
├── rigpulse.db
├── settings.json
├── logs/
└── reports/
```

**Skema awal SQL (sesuaikan migrasi):**

```sql
CREATE TABLE IF NOT EXISTS scan_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_type TEXT NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    status TEXT NOT NULL,
    summary_json TEXT
);

CREATE TABLE IF NOT EXISTS scan_findings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_run_id INTEGER NOT NULL,
    category TEXT NOT NULL,
    severity TEXT NOT NULL,
    status TEXT NOT NULL,
    title TEXT NOT NULL,
    explanation TEXT NOT NULL,
    evidence_json TEXT,
    recommended_action TEXT,
    FOREIGN KEY(scan_run_id) REFERENCES scan_runs(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS driver_operations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    update_id TEXT,
    device_label TEXT,
    source TEXT NOT NULL,
    action TEXT NOT NULL,
    state TEXT NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    error_message TEXT,
    restart_required INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS battery_health_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    collected_at TEXT NOT NULL,
    design_capacity_mwh INTEGER,
    full_charge_capacity_mwh INTEGER,
    health_percent REAL,
    source TEXT
);
```

- Aktifkan foreign keys SQLite; jalankan migrasi dengan transaksi.
- Simpan data seperlunya, hindari serial number, alamat MAC, atau hardware ID lengkap di riwayat tanpa kebutuhan kuat.
- `Delete History` benar-benar menghapus entri terkait; informasikan jika laporan ekspor tetap berada di folder pengguna.
- Default jangan menyimpan semua sampel 1 detik sepanjang hari. Gunakan bounded ring buffer di memory; persistence telemetry hanya bila enabled dan menerapkan retensi.
- DB error harus diberitahukan, tetapi tidak membuat proses scan perangkat gagal total jika hasilnya bisa tetap ditampilkan.

---

## 8. SECURITY, PRIVACY, & WINDOWS INTEGRATION

### Permissions

1. Start GUI non-elevated (asInvoker); read-only operations berfungsi sebisa mungkin tanpa UAC.
2. Request UAC **per operasi** yang butuh privilege, bukan sepanjang lifetime GUI.
3. Bila menggunakan helper elevated, helper harus bagian dari aplikasi terinstal, terverifikasi, input divalidasi ketat, memakai IPC aman, dan hanya menerima aksi yang di-allowlist. Jangan menjalankan command arbitrer dari input GUI.
4. Driver installation/update melalui sistem Windows yang resmi; jangan memaksa driver tidak kompatibel.
5. Bila System Restore tidak aktif/tersedia, berikan peringatan jelas. Jangan menjanjikan pengembalian selalu berhasil.
6. Jangan memberi privilege lebih dari yang dibutuhkan atau bypass corporate/group policy.

### Privacy

- Tidak ada akun, cloud, remote server, atau automatic telemetry bawaan.
- Logs lokal memakai redaksi data sensitif.
- Reports menyembunyikan identitas sensitif secara default.
- Buka URL dukungan vendor di browser hanya jika pengguna meminta.
- Sediakan penghapusan riwayat/log dan preferensi.

### Windows update integrity

- WUA menjadi otoritas update driver yang tersedia dalam mode otomatis v1.
- Hanya paket yang ditawarkan/diterima oleh mekanisme Windows bersangkutan, bukan heuristik versi string.
- Jangan mengklaim semua driver terbaru vendor bisa dicari oleh WUA.
- Driver OEM yang tidak ditawarkan WUA masuk kategori "Cek situs resmi", **bukan update otomatis**.
- Update All mengecualikan firmware/BIOS dan item policy-restricted.
- Restart diminta melalui dialog, **jangan memaksa reboot langsung**.

### Sensor limitation policy

- Jangan memakai WMI `MSAcpi_ThermalZoneTemperature` sebagai CPU package temperature tanpa bukti label dan kecocokan.
- Jangan menginstal custom kernel driver dari internet demi sensor.
- Tampilkan persis dari mana data berasal (`NVIDIA NVML`, `Windows Battery API`, dll.) di detail.
- Jika tidak tersedia, informasikan bahwa dukungan tergantung model/driver/sensor.

---

## 9. ERROR & EMPTY STATES

| Situasi | Perilaku UI |
|---|---|
| No internet | "Tidak ada koneksi. Monitoring lokal tetap tersedia." |
| Sensor suhu tidak ada | "Suhu tidak tersedia untuk perangkat ini." |
| NVML tidak terpasang | Tampilkan GPU model bila ada, telemetry unavailable |
| Desktop tanpa baterai | "Baterai tidak terdeteksi." |
| WUA gagal / diblokir policy | Error singkat + tindakan `Open Windows Update`/`Retry` jika layak |
| Driver update tidak ditemukan | "Tidak ada pembaruan driver yang ditawarkan saat ini." |
| Driver install gagal | Tampilkan sumber, kode error, saran langkah lanjut |
| Izin admin ditolak | Jangan install; "Instalasi dibatalkan." |
| Report export gagal | Jangan tampilkan success; beri pesan error + lokasi |
| Database corrupt/unavailable | Tetap tampilkan fungsi live bila memungkinkan dan beri peringatan |
| Metric tidak baru (stale) | Tampilkan waktu terakhir, tandai data usang |

Jangan menggunakan kata "aman", "sehat", "optimal" untuk hardware ketika belum memiliki bukti yang cukup.

---

## 10. DEVELOPMENT PLAN / MILESTONES

### PHASE 1 — Foundation & GUI (P0)

**Implement:** Cargo workspace, main GUI, theme tokens, icon system, sidebar, navigasi 8 halaman, settings minimal, structured logging, startup error handling, CI basic.

**Done when:** executable Windows membuka app dengan UI dark navy; semua menu berfungsi (halaman yang belum ada diberi label jelas); tampilan tidak hang; no fake data in production.

### PHASE 2 — Hardware & Battery Basics (P0)

**Implement:** WMI/Windows API hardware inventory, CPU/RAM/storage info, OS/BIOS, device model, battery level + supported capacities, copy specs.

**Done when:** nilai nyata sesuai perangkat uji dan data unsupported tertangani. Uji laptop dengan baterai dan desktop tanpa baterai.

### PHASE 3 — Live Monitoring (P0/P1)

**Implement:** sysinfo sampling, charts, bounded buffer, source metadata, optional NVIDIA NVML provider, capability probing, refresh scheduler.

**Done when:** CPU/RAM live values memperbarui tanpa UI freeze; grafik dan pause berjalan; suhu yang tidak tersedia tidak diisi angka dummy; CPU/Memory usage tests pass.

### PHASE 4 — Driver Scanner (P0)

**Implement:** enumerate devices, driver metadata, problem codes, search/filter, detail view; pemindaian update WUA hanya baca.

**Done when:** user dapat mengidentifikasi device bermasalah dan hasil WUA sebenar-benarnya, tanpa perubahan driver.

### PHASE 5 — Driver Installer (P1; highest-risk)

**Implement:** user consent dialog, download/update lewat WUA, allowlist validation, privilege flow, progress events, verification, restart UI, operation logs.

**Done when:** teruji di perangkat non-kritis/lab dengan persetujuan user, hasil dicek ulang, kesalahan/izin ditolak tertangani. BIOS/firmware dikecualikan.

### PHASE 6 — Diagnostics & Reports (P1)

**Implement:** device problem scan, event log, storage status bila tersedia, battery health findings, report JSON/PDF, history SQLite, privacy controls.

**Done when:** laporan dan temuan punya evidence/source, field sensitif disembunyikan default, hasil negatif dibatasi ke pemeriksaan yang benar-benar dilakukan.

### PHASE 7 — Packaging, QA, & Release (P0 release)

**Implement:** Windows release build, app icon/version, MSI using WiX, shortcuts, uninstall, installed resource paths, documented setup, smoke tests, checksums, code signing jika sertifikat tersedia.

**Done when:** laptop lain dapat memasang, membuka, menggunakan, menutup, dan uninstall aplikasi tanpa harus memasang Rust atau menggunakan terminal. Installer/EXE berjalan normal pada Windows 10/11 x64 yang didukung.

---

## 11. TEST PLAN

### Unit tests

- Battery health calculation: valid value, zero design capacity, missing, mismatched/relative units, cycle count unknown.
- Driver update state transitions: invalid transitions rejected, requires explicit consent.
- Device status normalization, unknown problem codes.
- Availability flags, stale timestamps.
- Severity mapping: tidak menyatakan "sehat" bila data incomplete.
- Report privacy: serial number/hardware IDs redacted by default.
- SQLite migration, transaction rollback, history retention.
- Metric ring buffer cap and deduplication.

### Integration tests

- Windows WMI queries return expected schema or graceful unsupported.
- Device enumerate via SetupAPI, no crash on disabled/unknown devices.
- WUA search status and error mapping, network failures.
- Elevated operations only after consent, cancel/deny UAC does not corrupt state.
- Export JSON/PDF creates valid files.
- Window resize / high DPI, locale, dark navy.
- Unsupported sensor provider does not break dashboard.

### Physical device test matrix

- Lenovo AMD + NVIDIA (contoh LOQ).
- ASUS Intel + NVIDIA.
- Acer Intel iGPU.
- HP AMD iGPU.
- Dell Intel.
- PC desktop tanpa baterai.
- Windows 10 x64 dan Windows 11 x64.

**Real-world driver testing:** lakukan hanya di test hardware/snapshot yang dapat dipulihkan; hindari menguji driver storage/chipset kritis secara sembrono. Dokumentasikan hardware yang dites, OS build, provider, langkah, hasil, keterbatasan.

### Definition of Done (global)

- [ ] Build `cargo check` sukses di Windows dengan toolchain terkunci.
- [ ] `cargo fmt --check`, `cargo clippy` (dengan warning policy yang layak), dan tests lolos.
- [ ] GUI tidak memblokir saat scan panjang.
- [ ] Semua fitur menampilkan real/supported data atau unsupported state.
- [ ] Tidak ada dummy scan data dalam release.
- [ ] Driver installs wajib consent + eligibility + status verification.
- [ ] Green update/install buttons konsisten, tidak ada emoji.
- [ ] Windows installer dapat diinstal dan di-uninstall.
- [ ] README/build/release/limitations tersedia.
- [ ] Pengujian lintas model Windows minimal terdokumentasi; belum diuji = belum diklaim didukung penuh.

---

## 12. BUILD & INSTALLER TARGET

**Pengembangan:** Windows dengan Rust + MSVC Build Tools + Windows SDK. Target default `x86_64-pc-windows-msvc`.

**Typical developer commands (bukan perintah untuk pengguna akhir):**

```powershell
rustup toolchain install stable
rustup default stable
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo build --release -p rigpulse
```

Nama package `rigpulse` harus diselaraskan dengan `Cargo.toml` yang sebenarnya.

**Packaging:** gunakan WiX Toolset + `cargo-wix` versi yang saling kompatibel untuk menghasilkan MSI, atau pipeline WiX langsung jika integrasi tersebut lebih stabil. Jangan hardcode command WiX major version yang tidak cocok. Installer perlu:

- Metadata product name RigPulse, publisher, version.
- Installed GUI executable, resource/icon, required runtimes.
- Start Menu shortcut; Desktop shortcut opsional.
- Uninstall support.
- Upgrade behavior (tidak menghapus preferensi pengguna tanpa pemberitahuan).
- App tidak meminta admin untuk monitoring biasa.
- Installer signing bila certificate tersedia; dokumentasikan jika unsigned build memicu SmartScreen.
- Checksum SHA-256 rilis.

CI release menggunakan Windows runner; jalankan formatting, tests, release binary, smoke checks semampunya, dan unggah artifact MSI + binary zip.

---

## 13. TASK ORDER FOR AI AGENT

Ikuti urutan berikut **tanpa menunda proof of functionality**:

1. Buat workspace dan tulis `PLAN.md` berisi todo fase terkini.
2. Buat app native minimal yang membuka window RigPulse dark navy.
3. Implement sidebar dan seluruh halaman dengan navigasi nyata.
4. Tambahkan providers data sistem dasar; hapus semua data placeholder dari production.
5. Bangun dashboard dari data store/application state yang nyata.
6. Tambahkan polling worker dan grafik realtime.
7. Implement hardware inventory dan battery monitoring yang graceful.
8. Tambahkan driver enumeration dan problem codes.
9. Tambahkan WUA update search read-only dan sumber update yang jelas.
10. Tambahkan update installation sebagai modul terpisah **setelah** safety tests/consent flow.
11. Tambahkan diagnostics, database, PDF/JSON, settings.
12. Validasi skenario unsupported/error/batteryless/offline.
13. Buat MSI, test install/uninstall, dokumentasi release.

**Saat merespons setiap fase, AI agent harus melaporkan:**

```text
Phase:
Implemented:
Files changed:
How to run/test:
Tests passed/failed:
Known limitations:
Next steps:
```

Jangan menyatakan "berhasil" tanpa build/test yang telah dijalankan. Jika bukan lingkungan Windows, jelaskan bahwa Win32 functionality belum bisa diverifikasi di perangkat Windows nyata.

---

## 14. OPEN RISKS & TECHNICAL DECISIONS

| Risk | Strategi mitigasi |
|---|---|
| Suhu CPU/GPU tidak universal | Provider modular + unsupported state |
| Fan RPM sering dibatasi firmware | Optional enhancement; jangan jadi syarat launch |
| WUA tidak menawarkan semua driver OEM | Jelaskan scope; official support link sebagai fallback |
| Driver install bisa gagal/restart | User consent, verification, logging, rollback bila ada |
| Pembacaan battery capacity tidak valid | Periksa unit/flags/zero/unknown |
| Vendor GPU APIs tidak tersedia | Graceful fallback |
| Perbedaan firmware/BIOS tiap brand | Capability detection, test matrix |
| Privilege escalation/security | Separate elevated capability/least privilege |
| GUI freezes karena WMI/COM | Workers, event channels, progress, cancellation |
| API/Crate berubah | Pin compatible versions; compile/test sebelum merge |
| SmartScreen untuk app unsigned | Signing resmi bila tersedia; panduan transparan |
| Diagnostic false positives | Evidence, confidence, source, conservative messaging |

### Launch priorities

- **P0:** App GUI, navigasi, hardware info, CPU/RAM usage, battery basics, driver inventory, WUA check, Windows packaging.
- **P1:** GPU telemetry, suhu sesuai dukungan, driver install, diagnostics, SQLite history, PDF/JSON reports.
- **P2:** localization complete, tray, autostart option, richer vendor sensors, advanced graphs, extended cross-brand support.

Jika suatu P1 berisiko tinggi, jangan memaksakan fitur tersebut dalam rilis awal. Lebih baik fitur belum tersedia tetapi diberi label benar daripada menginstal driver secara berbahaya.

---

## 15. PRIMARY TECHNICAL REFERENCES (VERIFIKASI SEBELUM IMPLEMENTASI)

- eframe: https://docs.rs/eframe/latest/eframe/
- egui: https://docs.rs/egui/latest/egui/
- sysinfo: https://docs.rs/sysinfo/latest/sysinfo/
- Windows API Rust: https://github.com/microsoft/windows-rs
- Microsoft WUA API: https://learn.microsoft.com/en-us/windows/win32/api/_wua/
- IUpdateSearcher::Search: https://learn.microsoft.com/en-us/windows/win32/api/wuapi/nf-wuapi-iupdatesearcher-search
- Windows CM_Get_DevNode_Status: https://learn.microsoft.com/en-us/windows/win32/api/cfgmgr32/nf-cfgmgr32-cm_get_devnode_status
- Windows driver selection: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/overview-of-the-driver-selection-process
- Windows battery information: https://learn.microsoft.com/en-us/windows/win32/power/battery-information-str
- Windows driver signing: https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/driver-signing-offerings
- cargo-wix: https://docs.rs/crate/cargo-wix/latest

**Sumber ini adalah rujukan desain, bukan jaminan bahwa seluruh API tersedia pada setiap hardware/Windows build.**

---

## 16. FINAL PROMPT TO THE AI CODING AGENT

> Implementasikan **RigPulse**, aplikasi native desktop Windows full Rust, mengikuti spesifikasi ini. Mulai dari **Phase 1**, buat workspace Cargo dan GUI egui/eframe yang berjalan, menggunakan dark navy background dan navigasi sederhana untuk pemula. Gunakan tombol Update/Install Driver berwarna hijau dan jangan gunakan emoji. Integrasikan data Windows sungguhan, bukan data dummy. Bekerja bertahap dengan test, error handling, capability detection, dan dokumentasi. Driver install tidak boleh berjalan tanpa persetujuan pengguna dan validasi sumber resmi; unsupported sensor ditampilkan jujur. Targetkan hasil akhir EXE GUI dan MSI installer kompatibel Windows 10/11 x64 pada laptop dengan vendor hardware berbeda. Setelah setiap fase, laporkan file yang dibuat, cara menjalankan, hasil tes, masalah, dan langkah berikutnya.

**END OF BLUEPRINT**
