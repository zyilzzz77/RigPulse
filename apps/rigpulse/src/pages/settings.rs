use crate::{app::RigPulseApp, theme, updater::State};
use eframe::egui::{self, RichText};
impl RigPulseApp {
    pub(crate) fn settings_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        theme::surface(ui, |ui| {
            theme::section(ui, "Umum");
            super::row(ui, "Bahasa aplikasi", "Bahasa Indonesia");
            ui.label(
                RichText::new("Preferensi disimpan secara lokal pada perangkat ini.")
                    .small()
                    .color(theme::MUTED),
            );
            if ui
                .button(RichText::new("Reset preferensi…").color(theme::DANGER))
                .clicked()
            {
                self.reset_confirm = true;
            }
        });
        theme::surface(ui, |ui| {
            theme::section(ui, "Pemantauan");
            ui.horizontal_wrapped(|ui| {
                ui.label("Interval pembacaan");
                egui::ComboBox::from_id_salt("refresh")
                    .selected_text(format!("{} detik", self.settings.refresh_seconds))
                    .show_ui(ui, |ui| {
                        for seconds in [1, 3, 5] {
                            ui.selectable_value(
                                &mut self.settings.refresh_seconds,
                                seconds,
                                format!("{seconds} detik"),
                            );
                        }
                    });
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Riwayat grafik awal");
                egui::ComboBox::from_id_salt("history_default")
                    .selected_text(format!("{} menit", self.settings.history_minutes))
                    .show_ui(ui, |ui| {
                        for minutes in [5, 15, 60] {
                            ui.selectable_value(
                                &mut self.settings.history_minutes,
                                minutes,
                                format!("{minutes} menit"),
                            );
                        }
                    });
            });
            theme::toggle(
                ui,
                &mut self.settings.power_saver,
                "Hemat daya — jeda pembacaan minimal 5 detik",
            );
            theme::toggle(
                ui,
                &mut self.settings.reduced_animations,
                "Kurangi animasi antarmuka",
            );
        });
        theme::surface(ui, |ui| {
            theme::section(ui, "Tampilan");
            super::row(ui, "Tema", "Dark Navy");
            ui.horizontal_wrapped(|ui| {
                ui.label("Ukuran antarmuka");
                egui::ComboBox::from_id_salt("ui_scale")
                    .selected_text(format!("{:.0}%", self.settings.interface_scale * 100.0))
                    .show_ui(ui, |ui| {
                        for scale in [0.8, 1.0, 1.25, 1.5, 1.75] {
                            ui.selectable_value(
                                &mut self.settings.interface_scale,
                                scale,
                                format!("{:.0}%", scale * 100.0),
                            );
                        }
                    });
            });
            ui.label(
                RichText::new(
                    "Ukuran ini mengikuti skala Windows. Perubahan diterapkan setelah disimpan.",
                )
                .small()
                .color(theme::MUTED),
            );
        });
        theme::surface(ui, |ui| {
            theme::section(ui, "Jaringan & pembaruan driver");
            ui.horizontal_wrapped(|ui| {
                ui.label("Riwayat jaringan");
                for n in [5, 15, 60] {
                    ui.selectable_value(
                        &mut self.settings.network_history_minutes,
                        n,
                        format!("{n} menit"),
                    );
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Target ping awal");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings.ping_target).desired_width(220.0),
                );
            });
            theme::toggle(
                ui,
                &mut self.settings.network_enabled,
                "Aktifkan pemantauan jaringan lokal",
            );
            theme::toggle(
                ui,
                &mut self.settings.external_lookup,
                "Izinkan pencarian IP publik manual ke ipify / ipwho.is",
            );
            ui.label("Penyedia menerima IP publik. Tidak ada nomor seri, daftar aplikasi, atau data hardware yang dikirim. Cache 1 jam; batas penyedia tetap berlaku.");
            super::row(
                ui,
                "Penyedia speedtest",
                "Speedtest by Ookla · CLI resmi · atas permintaan",
            );
            theme::toggle(
                ui,
                &mut self.settings.driver_notifications,
                "Notifikasi hasil operasi driver",
            );
            super::row(
                ui,
                "Konfirmasi instalasi driver",
                "Selalu diperlukan sebelum pemasangan",
            );
        });
        theme::surface(ui, |ui| {
            theme::section(ui, "Apps & Cleanup");
            theme::toggle(
                ui,
                &mut self.settings.apps_auto_scan,
                "Pindai sisa otomatis setelah Uninstall Lanjutan terverifikasi",
            );
            theme::toggle(
                ui,
                &mut self.settings.apps_background_scan,
                "Pemindaian ringan di latar belakang setiap 30 menit saat RigPulse terbuka",
            );
            theme::toggle(
                ui,
                &mut self.settings.apps_preserve_data,
                "Sembunyikan kandidat konfigurasi/data pengguna dari hasil pembersihan",
            );
            theme::toggle(
                ui,
                &mut self.settings.apps_registry_review,
                "Tinjau sisa registry — lanjutan, laporan saja",
            );
            super::row(
                ui,
                "Konfirmasi pembersihan",
                "Selalu diperlukan untuk setiap tindakan",
            );
            super::row(ui, "Retensi karantina", "Manual · batas 2 GiB / 2.500 file");
            ui.label(RichText::new("Pemindaian tidak menghapus file. Konfigurasi/data pengguna dan registry tidak dapat dihapus oleh pembersih generik. Izin administrator ditawarkan untuk mencoba ulang uninstaller desktop yang ditolak Windows; UAC tetap meminta persetujuan.").small().color(theme::MUTED));
        });
        theme::surface(ui, |ui| {
            theme::section(ui, "Privasi");
            theme::toggle(
                ui,
                &mut self.settings.include_sensitive,
                "Sertakan identitas sensitif dalam tampilan, salinan, dan laporan",
            );
            ui.label("Default mati. Mengaktifkannya dapat menyertakan nomor seri dan ID hardware. Data pemantauan dan laporan tetap lokal, tanpa akun atau telemetry.");
            ui.label("Hapus riwayat lokal melalui halaman Reports.");
        });
        theme::surface(ui, |ui| {
            theme::section(ui, "Tentang RigPulse");
            super::row(ui, "Versi terpasang", env!("CARGO_PKG_VERSION"));
            super::row(ui, "Jenis aplikasi", "Utilitas desktop Windows");
            super::row(ui, "Lisensi", env!("CARGO_PKG_LICENSE"));
            super::row(ui, "Pengembang / penerbit", env!("CARGO_PKG_AUTHORS"));
            theme::toggle(
                ui,
                &mut self.settings.auto_check_updates,
                "Periksa pembaruan aplikasi secara otomatis (setiap 24 jam)",
            );
            ui.label(
                RichText::new(
                    "Unduhan dimulai atas pilihan Anda. Instalasi selalu meminta persetujuan.",
                )
                .small()
                .color(theme::MUTED),
            );
            super::row(
                ui,
                "Pemeriksaan terakhir",
                &self
                    .settings
                    .last_app_update_check
                    .map(|at| {
                        at.with_timezone(&chrono::Local)
                            .format("%d/%m/%Y %H:%M:%S")
                            .to_string()
                    })
                    .unwrap_or_else(|| "Belum diperiksa".into()),
            );
            if ui
                .add_enabled(
                    !self.app_updater.busy(),
                    egui::Button::new("Periksa pembaruan aplikasi"),
                )
                .clicked()
            {
                self.app_updater.check(&mut self.settings, &self.dir, ctx);
            }
            match &self.app_updater.state {
                State::Idle => {
                    ui.label("Pilih periksa pembaruan untuk menghubungi sumber rilis resmi.");
                }
                State::Unconfigured => {
                    ui.colored_label(theme::WARNING, "Sumber rilis resmi belum dikonfigurasi.");
                    ui.label("Alamat server rilis dan kunci verifikasi penerbit belum tersedia dalam build ini. Pemeriksaan online dan unduhan belum dapat digunakan.");
                }
                State::Checking => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Memeriksa rilis resmi…");
                    });
                }
                State::Current => {
                    ui.colored_label(
                        theme::GREEN,
                        "Anda menggunakan versi terbaru pada kanal rilis resmi.",
                    );
                }
                State::Available(release) => {
                    let r = release.metadata();
                    ui.colored_label(theme::GREEN, format!("Versi {} tersedia", r.version));
                    super::row(
                        ui,
                        "Tanggal rilis",
                        &r.published_at
                            .with_timezone(&chrono::Local)
                            .format("%d/%m/%Y")
                            .to_string(),
                    );
                    super::row(
                        ui,
                        "Ukuran unduhan",
                        &format!("{:.1} MB", r.package.size as f64 / 1_000_000.0),
                    );
                    ui.label(&r.notes);
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Unduh pembaruan").color(theme::GREEN_TEXT),
                            )
                            .fill(theme::GREEN),
                        )
                        .clicked()
                    {
                        self.app_updater.download(&self.dir, ctx);
                    }
                }
                State::Downloading(n, total) => {
                    ui.add(
                        egui::ProgressBar::new(*n as f32 / (*total).max(1) as f32).text(format!(
                            "{:.1} / {:.1} MB",
                            *n as f64 / 1_000_000.0,
                            *total as f64 / 1_000_000.0
                        )),
                    );
                    if ui.button("Batalkan unduhan").clicked() {
                        self.app_updater.cancel();
                    }
                }
                State::Verifying => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Memverifikasi integritas paket…");
                    });
                }
                State::Ready(p) => {
                    ui.label(format!(
                        "Versi {} terverifikasi dan siap dipasang.",
                        p.release().version
                    ));
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Pasang pembaruan…").color(theme::GREEN_TEXT),
                            )
                            .fill(theme::GREEN),
                        )
                        .clicked()
                    {
                        self.app_updater.confirm = true;
                    }
                }
                State::Installing => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Ikuti jendela Windows Installer. Menunggu hasil instalasi…");
                    });
                }
                State::Installed(restart) => {
                    ui.label(if *restart{"Windows Installer selesai dan meminta restart Windows."}else{"Windows Installer selesai. Tutup lalu buka kembali RigPulse untuk memuat versi baru."});
                }
                State::Cancelled => {
                    ui.label("Operasi dibatalkan. Anda dapat memeriksa pembaruan lagi.");
                }
                State::Failed(error) => {
                    ui.colored_label(theme::DANGER,"Pembaruan belum berhasil. Periksa koneksi atau sumber rilis, lalu coba lagi.");
                    egui::CollapsingHeader::new("Detail kegagalan").show(ui, |ui| {
                        ui.label(error);
                    });
                }
            }
        });
        if ui.button("Simpan preferensi").clicked() {
            self.settings.normalize();
            self.notice = Some(match self.settings.save(&self.dir) {
                Ok(()) => {
                    self.minutes = self.settings.history_minutes;
                    theme::apply_scale(ctx, self.settings.interface_scale);
                    "Preferensi disimpan.".into()
                }
                Err(e) => format!("Preferensi gagal disimpan: {e}"),
            });
        }
        if self.reset_confirm {
            egui::Modal::new(egui::Id::new("reset_preferences")).show(ctx,|ui|{
                ui.set_max_width(420.0);ui.heading("Reset preferensi?");ui.label("Pengaturan kembali ke nilai awal. Riwayat pemeriksaan dan laporan tetap tersimpan.");
                ui.horizontal(|ui|{if ui.button("Batal").clicked(){self.reset_confirm=false;}
                    if ui.button(RichText::new("Reset dan simpan").color(theme::DANGER)).clicked(){
                        let defaults=rigpulse_core::Settings::default();
                        match defaults.save(&self.dir){Ok(())=>{self.settings=defaults;self.minutes=5;theme::apply_scale(ctx,1.0);self.notice=Some("Preferensi direset.".into());},Err(e)=>self.notice=Some(format!("Reset gagal: {e}"))};self.reset_confirm=false;
                    }
                });
            });
        }
        if self.app_updater.confirm {
            egui::Modal::new(egui::Id::new("app_install_consent")).show(ctx,|ui|{
                ui.set_max_width(440.0);ui.heading("Pasang pembaruan RigPulse?");
                ui.label("Paket sudah diverifikasi dengan tanda tangan penerbit dan SHA-256. Windows Installer akan dibuka. Simpan pekerjaan; Windows mungkin meminta menutup aplikasi atau restart. Tidak ada restart otomatis.");
                ui.horizontal(|ui|{if ui.button("Batal").clicked(){self.app_updater.confirm=false;}
                    if ui.add(egui::Button::new(RichText::new("Setuju, buka installer").color(theme::GREEN_TEXT)).fill(theme::GREEN)).clicked(){self.app_updater.install(ctx);}
                });
            });
        }
    }
}
