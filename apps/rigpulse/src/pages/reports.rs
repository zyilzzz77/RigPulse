use crate::{app::RigPulseApp, theme};
use eframe::egui::{self, RichText};
#[derive(Default)]
pub(crate) struct ReportView {
    revision: Option<u64>,
    metadata: Vec<String>,
    layout: crate::virtual_rows::MeasuredRows,
}
fn scan_card(ui: &mut egui::Ui, scan: &rigpulse_database::HistoryEntry, metadata: &str) -> bool {
    theme::surface(ui, |ui| {
        ui.label(RichText::new(&scan.status).strong());
        ui.label(metadata);
        ui.button("Buka hasil scan").clicked()
    })
}
impl RigPulseApp {
    pub(crate) fn reports_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        theme::surface(ui, |ui| {
            ui.label(RichText::new("Laporan perangkat").size(20.0).strong());
            ui.label(
                "Ekspor snapshot data yang telah dibaca. Setiap sumber memiliki timestamp sendiri.",
            );
            for (label, ready) in [
                ("Spesifikasi sistem", self.inventory.is_some()),
                ("Pemantauan", self.sample.is_some()),
                ("Baterai", self.battery.is_some()),
                ("Driver terpasang", self.devices.is_some()),
                ("Hasil diagnostik", self.diagnostics.is_some()),
            ] {
                super::row(
                    ui,
                    label,
                    if ready {
                        "Data tersedia untuk laporan"
                    } else {
                        "Belum tersedia; tidak akan dibuat-buat"
                    },
                );
            }
            theme::toggle(
                ui,
                &mut self.settings.include_sensitive,
                "Sertakan identitas sensitif (default mati)",
            );
            ui.horizontal_wrapped(|ui| {
                let ready =
                    !self.report_busy && (self.inventory.is_some() || self.diagnostics.is_some());
                if ui
                    .add_enabled(ready, egui::Button::new("Ekspor JSON"))
                    .clicked()
                {
                    self.export_report(false, ctx);
                }
                if ui
                    .add_enabled(ready, egui::Button::new("Ekspor PDF"))
                    .clicked()
                {
                    self.export_report(true, ctx);
                }
                if ui.button("Buka folder laporan").clicked() {
                    let path = self.dir.join("reports");
                    if let Err(e) = std::fs::create_dir_all(&path)
                        .map_err(anyhow::Error::from)
                        .and_then(|()| rigpulse_windows::shell::folder(&path))
                    {
                        self.notice = Some(e.to_string());
                    }
                }
                if self.report_busy {
                    ui.spinner();
                    ui.label("Membuat laporan…");
                }
            });
            if let Some(path) = &self.last_export {
                ui.label(
                    RichText::new(path.display().to_string())
                        .small()
                        .color(theme::MUTED),
                );
            }
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Riwayat scan").size(20.0).strong());
            if ui
                .add_enabled(
                    !self.db_busy && !self.diagnostic_busy,
                    egui::Button::new(RichText::new("Hapus riwayat").color(theme::DANGER)),
                )
                .clicked()
            {
                self.delete_confirm = true;
            }
        });
        if self.scans.is_empty() {
            ui.label("Belum ada riwayat tersimpan. Jalankan scan di Diagnostics.");
        }
        let mut load = None;
        if self.report_view.revision != Some(self.view_revision)
            || self.report_view.metadata.len() != self.scans.len()
        {
            self.report_view.metadata = self
                .scans
                .iter()
                .map(|scan| {
                    let at = chrono::DateTime::parse_from_rfc3339(&scan.at)
                        .map(|at| {
                            at.with_timezone(&chrono::Local)
                                .format("%d/%m/%Y %H:%M:%S")
                                .to_string()
                        })
                        .unwrap_or_else(|_| "Waktu tidak tersedia".into());
                    format!("{} · {} · #{}", scan.kind, at, scan.id)
                })
                .collect();
            self.report_view.layout = Default::default();
            self.report_view.revision = Some(self.view_revision);
        }
        self.report_view
            .layout
            .show(ui, "report_history", self.scans.len(), |index, ui| {
                let scan = &self.scans[index];
                if scan_card(ui, scan, &self.report_view.metadata[index]) {
                    load = Some(scan.id);
                }
            });
        if let Some(id) = load {
            self.load_scan(id, ctx);
        }
        if self.delete_confirm {
            egui::Window::new("Hapus riwayat?").collapsible(false).resizable(false).show(ctx,|ui|{ui.label("Hapus semua riwayat scan, operasi driver dan kesehatan baterai dari database lokal?");ui.label("File laporan yang sudah diekspor tetap berada di folder laporan.");ui.horizontal(|ui|{if ui.button("Batal").clicked(){self.delete_confirm=false;}
if ui.add(egui::Button::new("Hapus riwayat").fill(theme::DANGER)).clicked(){self.delete_confirm=false;self.delete_history(ctx);}});});
        }
    }
}
