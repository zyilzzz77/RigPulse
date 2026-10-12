use crate::{app::RigPulseApp, theme};
use eframe::egui::{self, RichText};
use rigpulse_core::Page;
mod apps;
mod diagnostics;
pub(crate) mod drivers;
mod monitoring;
mod network;
pub(crate) use network::NetworkView;
mod reports;
mod settings;
pub(crate) use drivers::DriverView;
pub(crate) use monitoring::ChartView;
pub(crate) use reports::ReportView;

#[derive(Default)]
pub(crate) struct HardwareView {
    key: Option<(u64, String, usize, bool)>,
    sections: Vec<(Vec<usize>, crate::virtual_rows::MeasuredRows)>,
}
impl HardwareView {
    fn refresh(
        &mut self,
        revision: u64,
        inventory: &rigpulse_core::Inventory,
        query: &str,
        category: usize,
        sensitive: bool,
    ) {
        let key = (revision, query.to_owned(), category, sensitive);
        if self.key.as_ref() == Some(&key) {
            return;
        }
        let query = query.to_lowercase();
        self.sections = inventory
            .sections
            .iter()
            .enumerate()
            .map(|(n, section)| {
                let indices = section
                    .properties
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| {
                        (category == 0 || category == n + 1)
                            && (!p.sensitive || sensitive)
                            && (query.is_empty()
                                || p.name.to_lowercase().contains(&query)
                                || p.value.to_lowercase().contains(&query))
                    })
                    .map(|(index, _)| index)
                    .collect();
                (indices, Default::default())
            })
            .collect();
        self.key = Some(key);
    }
}

pub fn row(ui: &mut egui::Ui, label: &str, value: &str) {
    theme::row(ui, label, value);
}
pub fn unavailable(ui: &mut egui::Ui, reason: &str) {
    ui.label(RichText::new("Tidak tersedia").strong());
    ui.label(RichText::new(reason).color(theme::MUTED));
}
pub fn percent(v: Option<u8>) -> String {
    v.map(|n| format!("{n}%"))
        .unwrap_or_else(|| "Tidak tersedia".into())
}
fn value(i: &rigpulse_core::Inventory, category: &str, suffix: &str) -> String {
    i.sections
        .iter()
        .filter(|s| s.name == category)
        .flat_map(|s| &s.properties)
        .filter(|p| !p.sensitive && p.name.ends_with(suffix))
        .map(|p| p.value.as_str())
        .collect::<Vec<_>>()
        .join(" / ")
}
pub(super) fn gpu_usage(s: &rigpulse_monitor::Sample) -> Option<&rigpulse_core::Measurement> {
    s.extra
        .iter()
        .find(|m| {
            m.value.is_some() && m.source == "Windows PDH / WDDM" && m.label.contains("penggunaan")
        })
        .or_else(|| {
            s.extra
                .iter()
                .find(|m| m.unit == "%" && m.label.ends_with("usage") && m.value.is_some())
        })
}
impl RigPulseApp {
    pub(crate) fn content(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        match self.page {
            Page::Dashboard => self.dashboard(ui, ctx),
            Page::Hardware => self.hardware(ui, ctx),
            Page::Battery => self.battery_page(ui, ctx),
            Page::Settings => self.settings_page(ui, ctx),
            Page::Monitoring => self.monitoring_page(ui),
            Page::Drivers => self.drivers_page(ui, ctx),
            Page::Apps => self.apps_page(ui, ctx),
            Page::Diagnostics => self.diagnostics_page(ui, ctx),
            Page::Reports => self.reports_page(ui, ctx),
        }
    }
    fn dashboard(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal_wrapped(|ui| {
            if self.busy {
                ui.spinner();
                ui.label("Memindai perangkat…");
            } else {
                ui.label("Status: pemantauan lokal aktif");
            }
            if let Some(s) = &self.sample {
                ui.label(
                    RichText::new(format!(
                        "Data terakhir {}",
                        s.cpu.at.with_timezone(&chrono::Local).format("%H:%M:%S")
                    ))
                    .small()
                    .color(theme::MUTED),
                );
            }
        });
        let count = theme::columns(ui.available_width(), 4);
        for start in (0..4).step_by(count) {
            ui.columns(count, |cols| {
                for (n, col) in cols.iter_mut().enumerate() {
                    let index = start + n;
                    let m = self.sample.as_ref().and_then(|s| match index {
                        0 => Some(&s.cpu),
                        1 => Some(&s.memory),
                        2 => gpu_usage(s),
                        _ => None,
                    });
                    let (title, desc, color) = [
                        ("Penggunaan CPU", "Beban prosesor saat ini", theme::BLUE),
                        ("Penggunaan RAM", "Memori yang digunakan", theme::PURPLE),
                        ("Penggunaan GPU", "Beban mesin grafis tersibuk", theme::CYAN),
                        ("Daya baterai", "Sisa daya untuk digunakan", theme::GREEN),
                    ][index];
                    let battery = self.battery.as_ref();
                    let (v, state, tooltip) = if index == 3 {
                        (
                            battery.and_then(|b| b.percent.map(f64::from)),
                            if battery.is_some_and(|b| b.present == Some(false)) {
                                "Tidak ada baterai"
                            } else if battery.is_some_and(|b| b.charging == Some(true)) {
                                "Mengisi daya"
                            } else {
                                "Status daya dari Windows"
                            },
                            "Persentase daya berbeda dari kesehatan baterai.",
                        )
                    } else if let Some(m) = m {
                        (
                            m.value,
                            if m.stale(chrono::Utc::now(), 15) {
                                "Data lama"
                            } else {
                                m.availability.label()
                            },
                            m.label.as_str(),
                        )
                    } else {
                        (
                            None,
                            "Menunggu data / sensor tidak tersedia",
                            "Model lengkap tersedia di Hardware.",
                        )
                    };
                    theme::metric(col, title, v, "%", desc, state, color, tooltip);
                }
            });
        }
        theme::surface(ui, |ui| {
            theme::section(ui, "Perangkat Anda");
            if let Some(i) = &self.inventory {
                for (label, category, suffix) in [
                    ("Produsen", "Perangkat", "Produsen"),
                    ("Model perangkat", "Perangkat", "Model perangkat"),
                    ("Sistem operasi", "Windows", "Edisi Windows"),
                    ("Prosesor", "CPU", "Nama"),
                    ("Memori terpasang", "Perangkat", "RAM yang dapat digunakan"),
                    ("Kartu grafis", "GPU & Display", "Nama"),
                    ("Arsitektur", "Windows", "Arsitektur"),
                ] {
                    let detected = value(i, category, suffix);
                    row(
                        ui,
                        label,
                        if detected.is_empty() {
                            "Tidak tersedia"
                        } else {
                            &detected
                        },
                    );
                }
            } else {
                ui.label("Spesifikasi sedang dibaca dari Windows.");
            }
        });
        theme::section(ui, "Aksi cepat");
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !self.busy && !self.diagnostic_busy,
                    egui::Button::new("Pindai sistem"),
                )
                .clicked()
            {
                self.scan(ctx);
                self.diagnose(ctx);
            }
            if ui.button("Detail hardware").clicked() {
                self.page = Page::Hardware;
            }
            if ui
                .add_enabled(
                    !self.wua_busy,
                    egui::Button::new("Periksa pembaruan driver"),
                )
                .clicked()
            {
                self.page = Page::Drivers;
                self.check_updates(ctx);
            }
        });
    }
    fn hardware(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!self.busy, egui::Button::new("Pindai ulang"))
                .clicked()
            {
                self.scan(ctx);
            }
            if ui
                .add_enabled(
                    self.inventory.is_some(),
                    egui::Button::new("Salin spesifikasi"),
                )
                .clicked()
                && let Some(i) = &self.inventory
            {
                ctx.copy_text(i.specifications(self.settings.include_sensitive));
                self.notice = Some("Spesifikasi disalin ke clipboard.".into());
            }
        });
        theme::toggle(
            ui,
            &mut self.settings.include_sensitive,
            "Sertakan identitas sensitif (nomor seri / ID hardware)",
        );
        ui.add(
            egui::TextEdit::singleline(&mut self.query)
                .desired_width(ui.available_width())
                .hint_text("Cari nama properti atau nilai spesifikasi…"),
        );
        ui.label(
            RichText::new(
                "GHz: kecepatan prosesor · GiB: kapasitas memori · GB: kapasitas penyimpanan.",
            )
            .small()
            .color(theme::MUTED),
        );
        if self.busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Memindai perangkat…");
            });
        }
        let Some(i) = &self.inventory else {
            unavailable(ui, "Spesifikasi belum tersedia. Coba pindai ulang.");
            return;
        };
        egui::ComboBox::from_id_salt("hardware_category")
            .selected_text(if self.category == 0 {
                "Semua kategori"
            } else {
                i.sections
                    .get(self.category - 1)
                    .map(|s| s.name.as_str())
                    .unwrap_or("Semua kategori")
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.category, 0, "Semua kategori");
                for (n, s) in i.sections.iter().enumerate() {
                    ui.selectable_value(&mut self.category, n + 1, &s.name);
                }
            });
        self.hardware_view.refresh(
            self.view_revision,
            i,
            &self.query,
            self.category,
            self.settings.include_sensitive,
        );
        let query = &self.query;
        let mut matches = 0;
        for (n, s) in i.sections.iter().enumerate() {
            if self.category != 0 && self.category != n + 1 {
                continue;
            }
            let (properties, layout) = &mut self.hardware_view.sections[n];
            if properties.is_empty() && (!query.is_empty() || s.error.is_none()) {
                continue;
            }
            matches += 1;
            theme::surface(ui, |ui| {
                egui::CollapsingHeader::new(RichText::new(&s.name).size(18.0).strong())
                    .id_salt(("hardware", n))
                    .default_open(true)
                    .show(ui, |ui| {
                        if let Some(e) = &s.error {
                            unavailable(ui, e);
                        }
                        layout.show(ui, "hardware_properties", properties.len(), |index, ui| {
                            let p = &s.properties[properties[index]];
                            row(ui, &p.name, &p.value);
                        });
                        egui::CollapsingHeader::new("Detail teknis")
                            .id_salt(("provider", n))
                            .show(ui, |ui| {
                                ui.label(&s.source);
                            });
                    });
            });
        }
        if matches == 0 {
            ui.label("Tidak ada spesifikasi yang cocok dengan pencarian.");
        }
    }
    fn battery_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if ui
            .add_enabled(!self.busy, egui::Button::new("Perbarui data baterai"))
            .clicked()
        {
            self.scan(ctx);
        }
        let Some(b) = &self.battery else {
            unavailable(ui, "Data baterai belum terbaca dari Windows.");
            return;
        };
        if b.present == Some(false) {
            theme::surface(ui, |ui| {
                theme::section(ui, "Tidak ada baterai terdeteksi");
                ui.label("Pemantauan perangkat lain tetap tersedia pada komputer ini.");
            });
            return;
        }
        theme::surface(ui, |ui| {
            ui.horizontal(|ui| {
                theme::icon(ui, Page::Battery, theme::GREEN);
                theme::section(ui, "Daya baterai");
            });
            ui.label(RichText::new(percent(b.percent)).size(36.0).strong());
            if let Some(p) = b.percent {
                ui.add(
                    egui::ProgressBar::new(f32::from(p) / 100.0)
                        .fill(theme::GREEN)
                        .desired_height(8.0),
                );
            }
            row(
                ui,
                "Status pengisian",
                match b.charging {
                    Some(true) => "Mengisi daya",
                    Some(false) => "Tidak mengisi",
                    None => "Informasi tidak tersedia",
                },
            );
            row(
                ui,
                "Sumber daya",
                match b.ac {
                    Some(true) => "Terhubung ke adaptor AC",
                    Some(false) => "Menggunakan baterai",
                    None => "Informasi tidak tersedia",
                },
            );
            row(
                ui,
                "Perkiraan waktu tersisa",
                &b.remaining_seconds
                    .map(|s| format!("{} jam {} menit", s / 3600, (s % 3600) / 60))
                    .unwrap_or_else(|| "Tidak tersedia".into()),
            );
            ui.label(
                RichText::new(format!(
                    "Data terakhir {}",
                    b.at.with_timezone(&chrono::Local).format("%H:%M:%S")
                ))
                .small()
                .color(theme::MUTED),
            );
        });
        theme::surface(ui, |ui| {
            theme::section(ui, "Daya masuk / keluar baterai");
            if let Some(watts) = b.watts() {
                ui.label(
                    RichText::new(format!("{:.1} W", watts.abs()))
                        .size(32.0)
                        .strong(),
                );
                ui.label(if watts > 0.0 {
                    "Sedang mengisi — daya masuk baterai"
                } else if watts < 0.0 {
                    "Menggunakan baterai — daya keluar"
                } else {
                    "Sensor melaporkan 0 W — tidak ada aliran daya bersih"
                });
                if b.packs.len() > 1 {
                    ui.label("Angka ini adalah jumlah daya bersih seluruh baterai yang terbaca.");
                }
            } else {
                ui.label("Tidak tersedia pada perangkat ini.");
            }
            ui.label(
                RichText::new(
                    "Sumber: Windows BATTERY_STATUS. Bukan daya total laptop atau rating adaptor.",
                )
                .small()
                .color(theme::MUTED),
            );
            ui.label(format!(
                "Pembaruan {}",
                b.at.with_timezone(&chrono::Local).format("%H:%M:%S")
            ));
            if b.watts().is_some() && self.battery_history.len() > 1 {
                egui::CollapsingHeader::new("Riwayat daya baterai").show(ui, |ui| {
                    let latest = self.battery_history.back().unwrap().0;
                    let points: Vec<[f64; 2]> = self
                        .battery_history
                        .iter()
                        .map(|(at, w)| {
                            [
                                (at.signed_duration_since(latest)).num_milliseconds() as f64
                                    / 1000.0,
                                *w,
                            ]
                        })
                        .collect();
                    egui_plot::Plot::new("battery_power")
                        .height(170.0)
                        .allow_scroll(false)
                        .include_y(0.0)
                        .x_axis_formatter(|mark, _| monitoring::time_label(mark.value))
                        .show(ui, |plot| {
                            plot.line(
                                egui_plot::Line::new(
                                    "Daya bersih (W): positif masuk, negatif keluar",
                                    points,
                                )
                                .color(theme::BLUE),
                            )
                        });
                });
            }
        });
        for (n, pack) in b.packs.iter().enumerate() {
            theme::surface(ui, |ui| {
                theme::section(ui, &format!("Kesehatan baterai {}", n + 1));
                row(
                    ui,
                    "Kimia",
                    pack.chemistry.as_deref().unwrap_or("Tidak tersedia"),
                );
                row(
                    ui,
                    "Tegangan",
                    &pack
                        .voltage
                        .map(|v| format!("{v:.2} V"))
                        .unwrap_or_else(|| "Tidak tersedia".into()),
                );
                row(
                    ui,
                    "Daya baterai ini",
                    &pack
                        .watts
                        .map(|w| format!("{w:+.1} W"))
                        .unwrap_or_else(|| "Tidak tersedia".into()),
                );
                row(
                    ui,
                    "Produsen baterai",
                    pack.manufacturer.as_deref().unwrap_or("Tidak tersedia"),
                );
                row(
                    ui,
                    "Model baterai",
                    pack.model.as_deref().unwrap_or("Tidak tersedia"),
                );
                ui.label(
                    RichText::new(
                        pack.health()
                            .map(|h| format!("{h:.1}% dari kapasitas awal"))
                            .unwrap_or_else(|| "Tidak tersedia".into()),
                    )
                    .size(24.0)
                    .strong(),
                );
                row(
                    ui,
                    "Kapasitas saat baru",
                    &pack
                        .design_mwh
                        .map(|v| format!("{:.2} Wh", f64::from(v) / 1000.0))
                        .unwrap_or_else(|| "Tidak tersedia".into()),
                );
                row(
                    ui,
                    "Kapasitas penuh sekarang",
                    &pack
                        .full_mwh
                        .map(|v| format!("{:.2} Wh", f64::from(v) / 1000.0))
                        .unwrap_or_else(|| "Tidak tersedia".into()),
                );
                row(
                    ui,
                    "Jumlah siklus",
                    &pack
                        .cycles
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "Tidak tersedia".into()),
                );
                ui.label(RichText::new("Kesehatan membandingkan kapasitas penuh sekarang dengan kapasitas saat baru. Angka ini berasal dari firmware dan dapat berubah setelah kalibrasi.").small().color(theme::MUTED));
            });
        }
        if b.packs.is_empty() {
            theme::surface(ui, |ui| {
                unavailable(
                    ui,
                    b.note.as_deref().unwrap_or(
                        "Kapasitas dan kesehatan baterai tidak dilaporkan oleh perangkat ini.",
                    ),
                );
            });
        }
        egui::CollapsingHeader::new("Detail teknis").show(ui,|ui|{ui.label("Sumber: Windows Battery IOCTL / GetSystemPowerStatus. Wh adalah kapasitas energi baterai.");});
    }
}

#[cfg(test)]
mod hardware_tests {
    use super::*;
    #[test]
    fn cached_hardware_respects_privacy_search_category_and_scan_refresh() {
        let mut inventory = rigpulse_core::Inventory {
            at: chrono::Utc::now(),
            sections: vec![rigpulse_core::HardwareSection {
                name: "CPU".into(),
                source: "test only".into(),
                error: None,
                properties: vec![
                    rigpulse_core::Property {
                        name: "Kecepatan".into(),
                        value: "4.2 GHz".into(),
                        sensitive: false,
                    },
                    rigpulse_core::Property {
                        name: "Serial".into(),
                        value: "private".into(),
                        sensitive: true,
                    },
                ],
            }],
        };
        let mut view = HardwareView::default();
        view.refresh(0, &inventory, "", 0, false);
        assert_eq!(view.sections[0].0, vec![0]);
        view.refresh(0, &inventory, "PRIVATE", 1, true);
        assert_eq!(view.sections[0].0, vec![1]);
        view.refresh(0, &inventory, "", 2, true);
        assert!(view.sections[0].0.is_empty());
        inventory.sections[0].properties[0].value = "5.1 GHz".into();
        view.refresh(1, &inventory, "5.1", 0, false);
        assert_eq!(view.sections[0].0, vec![0]);
    }
}
