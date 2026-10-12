use crate::{app::RigPulseApp, theme};
use eframe::egui::{self, RichText};
impl RigPulseApp {
    pub(crate) fn diagnostics_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        theme::surface(ui, |ui| {
            theme::section(ui, "Pemeriksaan yang tersedia");
            for (test, description) in [
                (
                    "Perangkat dan driver",
                    "Membaca status masalah yang dilaporkan Windows.",
                ),
                (
                    "Peristiwa Windows",
                    "Mencari peristiwa error dan kritis dalam rentang terpilih.",
                ),
                (
                    "Kesehatan baterai",
                    "Memvalidasi kapasitas yang dilaporkan firmware.",
                ),
                (
                    "Penyimpanan",
                    "Membaca status kesehatan yang tersedia dari Windows.",
                ),
            ] {
                super::row(ui, test, description);
            }
            if self.diagnostics.is_none() {
                ui.label(
                    RichText::new(
                        "Status: belum diuji. Pilih Pindai sistem untuk menjalankan pemeriksaan.",
                    )
                    .color(theme::MUTED),
                );
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Event Log:");
            for d in [1, 7, 30] {
                ui.selectable_value(&mut self.diagnostic_days, d, format!("{d} hari"));
            }
            if ui
                .add_enabled(!self.diagnostic_busy, egui::Button::new("Pindai sistem"))
                .clicked()
            {
                self.diagnose(ctx);
            }
            if self.diagnostic_busy {
                ui.spinner();
                ui.label("Memeriksa Windows tanpa mengubah sistem…");
            }
        });
        if let Some(run) = &self.diagnostics {
            theme::surface(ui, |ui| {
                ui.label(RichText::new(run.summary()).size(22.0).strong());
                ui.label(format!(
                    "{} temuan/status · {} · durasi {:.1} detik",
                    run.findings.len(),
                    run.finished_at
                        .with_timezone(&chrono::Local)
                        .format("%d/%m/%Y %H:%M:%S"),
                    (run.finished_at - run.started_at).num_milliseconds() as f64 / 1000.0
                ));
                ui.label("Hasil dibatasi pemeriksaan yang tercantum. Event historis bukan diagnosis otomatis kerusakan hardware.");
            });
            for (n, f) in run.findings.iter().enumerate() {
                theme::surface(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(
                            if f.severity == rigpulse_core::Severity::Critical {
                                theme::DANGER
                            } else {
                                theme::BLUE
                            },
                            match f.status {
                                rigpulse_core::FindingStatus::Unavailable => "Tidak tersedia",
                                rigpulse_core::FindingStatus::NotDetected => {
                                    "Tidak ditemukan pada pemeriksaan ini"
                                }
                                rigpulse_core::FindingStatus::Detected => match f.severity {
                                    rigpulse_core::Severity::Critical => "Masalah terdeteksi",
                                    rigpulse_core::Severity::Warning => "Perlu perhatian",
                                    rigpulse_core::Severity::Info => "Informasi",
                                },
                            },
                        );
                        ui.label(RichText::new(&f.category).strong());
                    });
                    ui.label(RichText::new(&f.title).size(18.0).strong());
                    ui.label(&f.explanation);
                    egui::CollapsingHeader::new("Bukti dan saran")
                        .id_salt(n)
                        .show(ui, |ui| {
                            ui.label(&f.evidence);
                            ui.label(format!("Saran: {}", f.recommended_action));
                        });
                });
            }
        } else {
            theme::surface(ui, |ui| {
                ui.label("Belum ada pemeriksaan diagnostik.");
                ui.label("Scan memeriksa problem code, Event Log System, kapasitas baterai, dan status storage yang tersedia.");
            });
        }
    }
}
