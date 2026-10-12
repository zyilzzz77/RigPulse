# RigPulse 0.4.0

Aplikasi desktop Windows x64 untuk mengenali perangkat, melihat penggunaan CPU/RAM/GPU, memeriksa driver dan membuat laporan. Ditulis dalam Rust dengan egui/eframe dan Windows API. Semua pembacaan produksi berasal dari perangkat/Windows; kegagalan pembacaan ditampilkan sebagai tidak tersedia.

## Memakai aplikasi

1. Buka `dist/RigPulse-0.4.0-x64.msi`, kemudian ikuti installer. Instalasi untuk pengguna saat ini, dengan shortcut Start Menu. File aplikasi berada di `%LOCALAPPDATA%\Programs\RigPulse`.
2. Alternatif tanpa installer: ekstrak ZIP portable atau jalankan `dist/RigPulse.exe`.
3. Dashboard menampilkan penggunaan CPU/RAM/GPU dan ringkasan komputer. Hardware menyediakan pencarian spesifikasi dan salin ke clipboard. Monitoring menyediakan grafik CPU/RAM 5, 15, atau 60 menit dan pembacaan GPU/sensor yang tersedia.
4. Drivers membaca perangkat dan problem code. `Periksa pembaruan driver` mencari melalui Windows Update Agent mengikuti kebijakan komputer. `Buka Windows Update` membuka pengaturan resmi Windows untuk pemasangan.
5. Diagnostics memeriksa secara lokal tanpa perbaikan otomatis. Reports membuka riwayat dan mengekspor JSON/PDF. Identitas sensitif disembunyikan secara default.
6. Apps membaca aplikasi desktop/MSI dan Store pengguna saat ini. Cari nama/penerbit, lihat detail, lalu uninstall melalui uninstaller resmi dengan konfirmasi. Uninstall Lanjutan dapat memindai sisa setelah penghapusan terverifikasi. Pembersih Sisa Aplikasi meninjau riwayat lokal dan menawarkan karantina cache yang memenuhi aturan. Pemulihan dan hapus permanen memerlukan konfirmasi terpisah.
7. Untuk internet, buka **Monitoring → Network**. Grafik menunjukkan lalu lintas adaptor saat ini; tes kecepatan internet terpisah ada di kartu **Internet Speedtest · Ookla**. Setujui ketentuan Ookla lalu tekan **Mulai Speedtest Ookla**. Download/upload, ping, jitter dan paket hilang mengikuti hasil asli; data yang tidak dilaporkan tetap tidak tersedia. Komponen resmi Ookla diunduh dan diverifikasi saat pertama digunakan.

Monitoring biasa berjalan dengan hak pengguna normal. Target Windows 10/11 x64 dengan driver grafis yang mendukung OpenGL untuk eframe/glow. Pengujian nyata saat ini pada Windows 11 Lenovo; Windows 10 dan merek lain belum divalidasi secara fisik. Rilis belum ditandatangani karena sertifikat signing tidak tersedia.

Jika uninstaller desktop/MSI gagal karena izin, tombol **Coba uninstaller dengan administrator** meminta persetujuan lalu membuka UAC Windows. Pembatalan UAC tidak dianggap berhasil. RigPulse tidak otomatis menjalankan seluruh aplikasi sebagai administrator.

Pembersih bersifat konservatif: cache VS Code, Discord dan Slack dengan nama/penerbit yang cocok dapat memenuhi syarat. Data pribadi, konfigurasi, aplikasi yang masih terinstal, folder bersama dan Windows dilindungi. Aplikasi lain hanya dapat dilaporkan; registry hanya ditinjau. Karantina belum membebaskan ruang disk sampai file dihapus permanen. Detail sumber, perlindungan dan batas: [Apps Manager](docs/APPS_MANAGER.md).

## GPU dan satuan

- Semua model/vendor yang dilaporkan DXGI dikenali tanpa daftar nama model hardcoded: NVIDIA GeForce RTX/GTX/Quadro, AMD Radeon, Intel Arc/UHD/Iris, serta adaptor lain.
- Windows WDDM/PDH menyediakan penggunaan engine tersibuk, memori dedicated yang dipakai, dan memori shared per adaptor bila driver menyediakan counter. Shared memory memakai RAM sistem; tidak menambah VRAM fisik.
- NVIDIA NVML opsional menyediakan penggunaan, suhu, VRAM, daya (W), clock (GHz), dan RPM kipas bila GPU/driver mendukungnya. DLL NVIDIA tidak dibundel; dibaca dari lokasi instalasi sistem yang dipercaya. Tidak ada overclock atau perubahan pengaturan GPU.
- Suhu AMD/Intel dan suhu inti CPU belum memiliki provider terverifikasi. Sensor yang tidak tersedia tetap berlabel jelas. Pengujian RTX nyata dilakukan pada GPU host; model/vendor lain belum diuji secara fisik.
- GHz menjelaskan kecepatan prosesor; angka WMI merupakan laporan Windows, bukan pengukuran turbo kontinu. GiB digunakan untuk memori, GB untuk kapasitas disk, Hz untuk refresh layar, dan Wh untuk kapasitas baterai.

## Status pemasangan driver

Pencarian, Download Selected, Install Selected dan Update All memakai Windows Update Agent resmi mengikuti kebijakan komputer. Paket terpilih memerlukan persetujuan dan validasi ulang ID/revisi/kelas. Firmware/BIOS, kelas kritis dan lisensi yang belum diterima dikecualikan. Pemasangan memerlukan administrator: tombol **Kelola driver dengan izin admin** membuka jendela Drivers khusus melalui UAC; pilih dan konfirmasikan paket di sana. Aplikasi tidak memasang driver atau me-restart komputer otomatis. Progres/ETA hanya mengikuti data WUA yang tersedia. Windows Update tetap dapat dibuka bila API/kebijakan membatasi operasi.

Pemasangan nyata, UAC interaktif dan restart belum diuji secara destruktif dalam pengujian otomatis. Lihat [laporan lengkap](docs/COMPLETE_UPGRADE.md) untuk batas verifikasi dan perlindungan.

## Data lokal dan privasi

`%LOCALAPPDATA%\RigPulse\` berisi `settings.json`, `rigpulse.db`, `logs\rigpulse.log`, dan `reports\`. Tidak membutuhkan akun atau telemetry. Inventaris/monitoring/diagnosis bekerja lokal; Windows Update membutuhkan layanan/koneksi yang sesuai.

Apps menambahkan `apps/history.json` untuk riwayat uninstall/verifikasi, `apps/quarantine/` untuk file karantina dan `apps/quarantine.json` untuk jurnal pemulihan. Karantina memiliki retensi manual, maksimal 2 GiB/2.500 item, dan hanya mendukung pemindahan pada volume yang sama. Scan latar belakang opsional berjalan setiap 30 menit selama RigPulse terbuka; default mati. Scan setelah uninstall lanjutan default aktif, data pengguna dilindungi, tinjauan registry default mati. Simpan preferensi di Settings.

`Hapus riwayat` menghapus riwayat scan/operasi driver/kesehatan baterai setelah konfirmasi. Laporan yang telah diekspor tetap ada. Uninstall/upgrade mempertahankan preferensi, database, log dan laporan. Serial/PnP/hardware ID tidak disertakan dalam clipboard/laporan kecuali dipilih pengguna. Diagnosis tidak menyalin isi EventData Windows yang berpotensi sensitif.

## Build dari source

Prasyarat: Rust 1.90.0 MSVC x64, Visual Studio C++ Build Tools, Windows SDK. Proyek memakai Cargo.lock dan static CRT. Untuk toolchain lokal workspace, jalankan `. ./scripts/dev-env.ps1` dahulu.

```powershell
cargo fmt --all --check
cargo check --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
cargo run -p rigpulse --locked
cargo build --release -p rigpulse --locked
```

Untuk MSI, gunakan WiX **3.14.1** portable dari [rilis resmi WiX](https://github.com/wixtoolset/wix3/releases/tag/wix3141rtm), lalu `./scripts/package.ps1 -WixBin <folder-WiX>`. Script menghasilkan EXE, MSI, ZIP, lisensi dependency dan `SHA256SUMS.txt`. Feature screenshot tidak diaktifkan dalam packaging. Instalasi driver resmi tersedia dalam build produksi dengan konfirmasi dan izin Windows. Logo `RigPulse Neon Monitoring Logo.png` dibenamkan ke EXE dan digunakan oleh jendela/shortcut.

Tes integrasi jaringan/hardware dijalankan terpisah:

```powershell
cargo test -p rigpulse-drivers real_wua_read_only -- --ignored
cargo test -p rigpulse-windows actual_dxgi_and_wddm_counters -- --ignored
cargo run -p rigpulse-reports --example qa_export -- artifacts/qa-report
```

Catatan fase: [docs/PHASE_LOG.md](docs/PHASE_LOG.md). Arsitektur: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Bukti pengujian: [docs/TEST_PLAN.md](docs/TEST_PLAN.md). Batas dukungan: [docs/LIMITATIONS.md](docs/LIMITATIONS.md).

## Pembaruan versi 0.2.0

Antarmuka Dark Navy kini memakai sidebar tetap dengan logo, kartu Dashboard CPU/RAM/GPU/baterai responsif, baris spesifikasi yang menumpuk pada jendela sempit, grafik CPU/RAM terpisah atau gabungan, label waktu yang mudah dimengerti, serta daftar driver yang hanya merender baris terlihat. Jeda grafik membekukan tampilan; pengumpulan data langsung tetap berjalan.

Settings menyediakan interval 1/3/5 detik, riwayat awal 5/15/60 menit, hemat daya, pengurangan animasi, ukuran antarmuka, privasi, reset dengan konfirmasi, dan Tentang RigPulse. Simpan preferensi untuk mempertahankan pilihan setelah restart.

**Updater aplikasi belum dapat digunakan secara online:** proyek belum memiliki alamat rilis resmi dan kunci penandatanganan penerbit. Aplikasi menampilkan kondisi tersebut secara jujur. Backend HTTPS, manifest bertanda tangan Ed25519, perbandingan versi, download/progress/cancel, SHA-256, validasi identitas/arsitektur MSI, persetujuan dan eksekusi Windows Installer telah diimplementasikan. Jalur online dari awal hingga instalasi belum dikonfigurasi/diverifikasi. Detail konfigurasi penerbit: [docs/APPLICATION_UPDATES.md](docs/APPLICATION_UPDATES.md). Audit dan daftar perubahan: [docs/UI_UX_OVERHAUL.md](docs/UI_UX_OVERHAUL.md).

## Complete Upgrade — Phase 1 (0.3.1)

Build ini mengoptimalkan scroll Hardware/Reports, cache filter driver dan grafik, serta beban panel detail Apps. Daftar produksi tetap memakai data Windows asli. Detail perubahan, pengukuran dan panduan uji: [Phase 1](docs/UPGRADE_PHASE1.md). Phase 1 telah diterima pengguna. Phase 2–6 dilanjutkan pada versi 0.4.0; lihat laporan upgrade berikut.

## Complete Upgrade — Phase 2–6 (0.4.0)

- Apps: popup detail terpusat pada semua ukuran, konfirmasi uninstall terpisah, notifikasi 5 detik, refresh daftar setelah penghapusan terverifikasi, dan popup tinjauan sisa sebelum karantina.
- Battery: daya masuk/keluar dalam W dari sensor Windows, tegangan V, kimia dan riwayat daya; unknown tidak berubah menjadi nol. Daya baterai dibedakan dari rating charger/daya seluruh laptop.
- Monitoring → Network: adaptor/rute Windows, download/upload langsung, Mbps atau B/s, grafik 5/15/60 menit, jeda, total dan rata-rata berdasarkan interval terukur. IP/MAC/SSID tersembunyi secara default. Pembacaan SSID/sinyal bersifat manual dan mengikuti izin lokasi Windows.
- Speedtest menggunakan **CLI resmi Ookla** melalui modul Rust. Komponen proprietary tidak dibundel dalam MSI. Saat pertama dipakai, setujui EULA/ketentuan/privasi Ookla lalu mulai tes; unduhan resmi dan executable diverifikasi SHA-256. Tes memakai kuota internet, hanya atas permintaan, dengan pembatalan dan batas waktu 120 detik. Trafik terlapor di atas 512 MiB menghentikan tes; batas ini bukan penghitung tepat semua byte jaringan. Hasil asli menampilkan server, download, upload, ping dan jitter bila tersedia. Ketentuan CLI mengatur penggunaan pribadi/nonkomersial.
- Ping memakai ICMP Windows IPv4/IPv6. Pencarian IP publik/ISP/ASN manual memerlukan izin Settings dan memakai HTTPS ipify/ipwho.is dengan cache satu jam. Tidak ada data hardware yang dikirim. Data jaringan tidak masuk laporan.
- Settings tambahan tersimpan secara lokal; preferensi lama tetap dimigrasikan. Logo pengguna dan tampilan Dark Navy tetap digunakan.

Metode, 18 butir laporan, file terkait, hasil pengujian dan batas dukungan: [COMPLETE_UPGRADE.md](docs/COMPLETE_UPGRADE.md). File installer terbaru: `dist/RigPulse-0.4.0-x64.msi`.
