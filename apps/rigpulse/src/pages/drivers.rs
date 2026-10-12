use crate::{app::RigPulseApp, theme};
use eframe::egui::{self, RichText};
use rigpulse_core::{DeviceDriverInfo, DeviceStatus};

#[derive(Default)]
pub(crate) struct DriverView {
    key: Option<(u64, usize, String, String, usize)>,
    indices: Vec<usize>,
    issues: usize,
}
impl DriverView {
    fn refresh(
        &mut self,
        revision: u64,
        devices: &[DeviceDriverInfo],
        query: &str,
        class: &str,
        status: usize,
    ) {
        if self.key.as_ref().is_some_and(|key| {
            key.0 == revision
                && key.1 == devices.len()
                && key.2 == query
                && key.3 == class
                && key.4 == status
        }) {
            return;
        }
        let search = query.to_lowercase();
        self.issues = devices.iter().filter(|d| attention(&d.status)).count();
        self.indices = devices
            .iter()
            .enumerate()
            .filter(|(_, d)| {
                status_matches(&d.status, status)
                    && (class.is_empty() || category(&d.class) == class)
                    && (search.is_empty()
                        || format!(
                            "{} {} {} {} {}",
                            d.name,
                            d.class,
                            category(&d.class),
                            d.manufacturer.as_deref().unwrap_or(""),
                            d.version.as_deref().unwrap_or("")
                        )
                        .to_lowercase()
                        .contains(&search))
            })
            .map(|(index, _)| index)
            .collect();
        self.key = Some((
            revision,
            devices.len(),
            query.to_owned(),
            class.to_owned(),
            status,
        ));
    }
}

impl RigPulseApp {
    pub(crate) fn drivers_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal_wrapped(|ui| {
            if !rigpulse_drivers::privilege::is_admin() {
                if ui.add_enabled(!self.admin_busy&&!self.install_busy&&!self.wua_busy,egui::Button::new("Kelola driver dengan izin admin…")).clicked(){self.open_driver_admin(ctx);}
            } else if self.driver_admin {ui.colored_label(theme::WARNING,"Pengelola driver · izin administrator");}
            if self.admin_busy {ui.label("Selesaikan pengelolaan driver di jendela administrator. Daftar diperbarui setelah jendela ditutup.");}
            if ui
                .add_enabled(!self.busy, egui::Button::new("Pindai perangkat"))
                .clicked()
            {
                self.scan(ctx);
            }
            if ui
                .add_enabled(
                    !self.wua_busy && !self.install_busy && !self.admin_busy,
                    egui::Button::new("Periksa pembaruan driver"),
                )
                .clicked()
            {
                self.check_updates(ctx);
            }
            if ui.button("Buka Windows Update").clicked()
                && let Err(e) = rigpulse_windows::shell::windows_update()
            {
                self.notice = Some(e.to_string());
            }
        });
        let count = self
            .devices
            .as_ref()
            .map(|d| d.len().to_string())
            .unwrap_or_else(|| "—".into());
        self.driver_view.refresh(
            self.view_revision,
            self.devices.as_deref().unwrap_or_default(),
            &self.device_query,
            &self.driver_class,
            self.driver_status,
        );
        let issues = if self.devices.is_some() {
            self.driver_view.issues.to_string()
        } else {
            "—".into()
        };
        let available = if self.wua_busy {
            "Memeriksa…".into()
        } else if self.wua_error.is_some() {
            "Tidak tersedia".into()
        } else {
            self.updates
                .as_ref()
                .map(|s| s.updates.len().to_string())
                .unwrap_or_else(|| "Belum diperiksa".into())
        };
        let checked = self
            .updates
            .as_ref()
            .map(|s| {
                s.at.with_timezone(&chrono::Local)
                    .format("%d/%m %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|| "Belum pernah".into());
        theme::surface(ui, |ui| {
            let columns = if ui.available_width() >= 700.0 {
                4
            } else if ui.available_width() >= 280.0 {
                2
            } else {
                1
            };
            let stats = [
                ("Perangkat", count),
                ("Perlu perhatian", issues),
                ("Pembaruan", available),
                ("Terakhir diperiksa", checked),
            ];
            for row in stats.chunks(columns) {
                ui.columns(columns, |cols| {
                    for (col, (label, value)) in cols.iter_mut().zip(row) {
                        col.label(RichText::new(*label).small().color(theme::MUTED));
                        col.label(
                            RichText::new(value)
                                .size(if value.len() > 8 { 16.0 } else { 25.0 })
                                .strong(),
                        );
                    }
                });
            }
        });
        egui::CollapsingHeader::new("Pembaruan resmi dari Windows Update")
            .default_open(
                self.wua_busy
                    || self.wua_error.is_some()
                    || self.updates.as_ref().is_some_and(|s| !s.updates.is_empty()),
            )
            .show(ui, |ui| self.driver_updates_panel(ui));
        if self.install_confirm {
            let modal=egui::Modal::new(egui::Id::new("driver_consent")).show(ctx,|ui|{
                ui.set_width((ctx.screen_rect().width()*0.9-48.0).clamp(260.0,600.0));
                ui.heading(if self.download_only{"Konfirmasi unduhan driver"}else{"Konfirmasi pemasangan driver"});
                egui::ScrollArea::vertical().max_height(ctx.screen_rect().height()*0.65).show(ui,|ui|{
                ui.label("Sumber: Windows Update Agent. Paket akan divalidasi ulang sebelum download.");
                if let Some(search)=&self.updates{for u in &search.updates{if self.install_selection.contains(&u.id){ui.label(format!("{} / {} / {}",u.model,u.provider,u.title));}}}
                ui.label(if self.download_only{"Paket diunduh ke cache resmi Windows Update. Driver belum dipasang pada langkah ini."}else{"Pembaruan dapat mengganggu sementara tampilan atau jaringan dan memerlukan restart. Firmware/BIOS dan kelas kritis dikecualikan. Simpan pekerjaan sebelum melanjutkan."});
                ui.label("Tidak ada restart otomatis. Titik pemulihan tidak dibuat otomatis oleh RigPulse.");
                theme::toggle(ui,&mut self.install_ack,if self.download_only{"Saya memilih dan menyetujui unduhan paket ini"}else{"Saya memilih dan menyetujui unduhan serta pemasangan paket ini"});
                ui.horizontal(|ui|{if ui.button("Batal").clicked(){self.install_confirm=false;self.install_ack=false;}
if ui.add_enabled(self.install_ack&&!self.install_busy,egui::Button::new(RichText::new(if self.download_only{"Unduh driver"}else{"Pasang driver"}).color(theme::GREEN_TEXT)).fill(theme::GREEN)).clicked(){
                    let consent=self.updates.as_ref().ok_or_else(||anyhow::anyhow!("Hasil pencarian tidak tersedia")).and_then(|s|rigpulse_drivers::install::Consent::confirm(s,&self.install_selection,self.install_ack));
                    self.install_confirm=false;self.install_ack=false;match consent{Ok(c)=>self.start_install(c,ctx),Err(e)=>self.notice=Some(e.to_string())}
                }});
                });
            });
            if modal.should_close() {
                self.install_confirm = false;
                self.install_ack = false;
            }
        }
        if self.restart_confirm {
            let modal=egui::Modal::new(egui::Id::new("driver_restart")).show(ctx,|ui|{ui.set_max_width(420.0);ui.heading("Restart Windows sekarang?");ui.label("Simpan semua pekerjaan terlebih dahulu. Windows akan langsung dimulai ulang setelah Anda menekan tombol konfirmasi.");ui.horizontal_wrapped(|ui|{if ui.button("Batal / nanti").clicked(){self.restart_confirm=false;}
if ui.button("Ya, restart Windows").clicked(){self.restart_confirm=false;if let Err(e)=rigpulse_drivers::privilege::restart_confirmed(){self.notice=Some(e.to_string());}}});});
            if modal.should_close() {
                self.restart_confirm = false;
            }
        }

        theme::section(ui, "Daftar perangkat");
        ui.add(
            egui::TextEdit::singleline(&mut self.device_query)
                .desired_width(ui.available_width())
                .min_size(egui::vec2(0.0, 36.0))
                .margin(egui::Margin::symmetric(12, 8))
                .hint_text("Cari perangkat, produsen, atau versi driver…"),
        );
        let filter_width = ((ui.available_width() - 16.0) / 2.0).clamp(100.0, 180.0);
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("driver_category")
                .width(filter_width)
                .selected_text(if self.driver_class.is_empty() {
                    "Semua kategori"
                } else {
                    &self.driver_class
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.driver_class, String::new(), "Semua kategori");
                    for category in [
                        "Grafis",
                        "Audio",
                        "Jaringan",
                        "Bluetooth",
                        "Penyimpanan",
                        "Sistem",
                        "Lainnya",
                    ] {
                        ui.selectable_value(&mut self.driver_class, category.into(), category);
                    }
                });
            egui::ComboBox::from_id_salt("driver_status")
                .width(filter_width)
                .selected_text(filter_label(self.driver_status))
                .show_ui(ui, |ui| {
                    for n in 0..6 {
                        ui.selectable_value(&mut self.driver_status, n, filter_label(n));
                    }
                });
            if (!self.device_query.is_empty()
                || !self.driver_class.is_empty()
                || self.driver_status != 0)
                && ui.button("Reset filter").clicked()
            {
                self.device_query.clear();
                self.driver_class.clear();
                self.driver_status = 0;
            }
        });
        let Some(devices) = &self.devices else {
            ui.label(if self.busy {
                "Memindai perangkat…"
            } else {
                "Daftar perangkat belum terbaca. Pilih Pindai perangkat."
            });
            return;
        };
        self.driver_view.refresh(
            self.view_revision,
            devices,
            &self.device_query,
            &self.driver_class,
            self.driver_status,
        );
        let filtered = &self.driver_view.indices;
        ui.label(
            RichText::new(format!(
                "Menampilkan {} dari {} perangkat",
                filtered.len(),
                devices.len()
            ))
            .small()
            .color(theme::MUTED),
        );
        if filtered.is_empty() {
            theme::surface(ui, |ui| {
                ui.strong("Tidak ada perangkat yang cocok");
                ui.label("Ubah kata pencarian atau reset filter untuk menampilkan perangkat lain.");
            });
        } else {
            render_table(ui, devices, filtered, &mut self.selected_device);
        }
        if let Some(n) = self.selected_device
            && let Some(d) = devices.get(n)
        {
            let mut open = true;
            egui::Window::new("Detail perangkat").open(&mut open).default_width(520.0).max_width((ctx.screen_rect().width()-32.0).max(280.0)).show(ctx,|ui| {
                egui::ScrollArea::vertical().max_height(ctx.screen_rect().height()-160.0).show(ui,|ui| {
                    ui.add(egui::Label::new(RichText::new(&d.name).size(19.0).strong()).wrap().selectable(true));
                    for (label,text) in [("Kategori",category(&d.class)),("Produsen",d.manufacturer.as_deref().unwrap_or("Tidak tersedia")),("Penyedia driver",d.provider.as_deref().unwrap_or("Tidak tersedia")),("Versi terpasang",d.version.as_deref().unwrap_or("Tidak tersedia")),("Tanggal driver",d.date.as_deref().unwrap_or("Tidak tersedia")),("Status",status(&d.status))] {super::row(ui,label,text);}
                    theme::toggle(ui,&mut self.settings.include_sensitive,"Tampilkan ID hardware sensitif");
                    if self.settings.include_sensitive {super::row(ui,"ID perangkat",&d.instance_id);for id in &d.hardware_ids {super::row(ui,"ID hardware",id);}}
                    if ui.button("Salin detail").clicked(){
                        let mut text=format!("{}\nKategori: {}\nVersi: {}\nPenyedia: {}\nStatus: {}",d.name,category(&d.class),d.version.as_deref().unwrap_or("Tidak tersedia"),d.provider.as_deref().unwrap_or("Tidak tersedia"),status(&d.status));
                        if self.settings.include_sensitive {text.push_str(&format!("\nID: {}\n{}",d.instance_id,d.hardware_ids.join("\n")));}
                        ctx.copy_text(text);self.notice=Some("Detail perangkat disalin.".into());
                    }
                    ui.label("Status perangkat dan ketersediaan pembaruan diperiksa secara terpisah.");
                    egui::CollapsingHeader::new("Detail teknis").show(ui,|ui| {super::row(ui,"Kelas Windows",&d.class);super::row(ui,"Kode masalah",&d.problem.map(|v|v.to_string()).unwrap_or_else(||"Tidak tersedia".into()));});
                });
            });
            if !open {
                self.selected_device = None;
            }
        }
    }
    fn driver_updates_panel(&mut self, ui: &mut egui::Ui) {
        theme::surface(ui, |ui| {
            ui.label("Pembaruan hanya berasal dari Windows Update dan mengikuti kebijakan perangkat ini.");
            if self.wua_busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Memeriksa pembaruan resmi… Proses dapat memerlukan beberapa menit.");
                });
                if let Some(p) = &self.search_progress {
                    ui.label(format!(
                        "Berlalu {:.0} detik · perkiraan waktu belum tersedia",
                        p.elapsed
                    ));
                    if p.abort_requested {
                        ui.label("Meminta WUA membatalkan pencarian…");
                    }
                }
                if ui.button("Batalkan pencarian").clicked() {
                    self.wua_cancel
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                }
            } else if let Some(error) = &self.wua_error {
                ui.colored_label(
                    theme::DANGER,
                    "Pemeriksaan belum berhasil. Periksa koneksi dan pengaturan Windows Update.",
                );
                egui::CollapsingHeader::new("Detail kegagalan").show(ui, |ui| {
                    ui.label(error);
                });
            } else if let Some(search) = &self.updates {
                if search.updates.is_empty() {
                    ui.label("Tidak ada pembaruan driver yang tersedia dari pemeriksaan terakhir.");
                }
                if let Some(warning) = &search.warning {
                    ui.label(warning);
                }
                for update in &search.updates {
                    ui.separator();
                    ui.add(egui::Label::new(RichText::new(&update.title).strong()).wrap());
                    ui.label(
                        RichText::new(format!("{} · {}", update.model, update.provider))
                            .small()
                            .color(theme::MUTED),
                    );
                    if let Some(reason) = &update.reason {
                        ui.label(reason);
                    }
                    ui.horizontal_wrapped(|ui| {
                        let mut selected = self.install_selection.contains(&update.id);
                        if ui
                            .add_enabled(
                                update.eligible && !self.install_busy,
                                egui::Checkbox::new(&mut selected, "Pilih paket"),
                            )
                            .changed()
                        {
                            if selected {
                                self.install_selection.push(update.id.clone());
                            } else {
                                self.install_selection.retain(|id| id != &update.id);
                            }
                        }
                        ui.label(if update.downloaded {
                            "Sudah diunduh oleh Windows"
                        } else {
                            "Belum diunduh"
                        });
                    });
                    egui::CollapsingHeader::new(format!("Detail paket · {}", update.id)).show(
                        ui,
                        |ui| {
                            super::row(
                                ui,
                                "Update ID / revisi",
                                &format!("{} / {}", update.id, update.revision),
                            );
                            super::row(
                                ui,
                                "Ukuran unduhan maksimum",
                                &update
                                    .max_download_bytes
                                    .map(|b| format!("{:.1} MB", b / 1_000_000.0))
                                    .unwrap_or_else(|| "Tidak tersedia".into()),
                            );
                            super::row(
                                ui,
                                "Kemungkinan restart",
                                match update.restart_possible {
                                    Some(true) => "Dapat diperlukan",
                                    Some(false) => "Tidak dilaporkan perlu",
                                    None => "Tidak tersedia",
                                },
                            );
                            ui.label(
                                update
                                    .description
                                    .as_deref()
                                    .unwrap_or("Deskripsi tidak tersedia"),
                            );
                            if self.settings.include_sensitive {
                                super::row(ui, "Hardware ID", &update.hardware_id);
                            }
                        },
                    );
                    let enabled = update.eligible
                        && rigpulse_drivers::privilege::is_admin()
                        && !self.admin_busy
                        && !self.install_busy
                        && !self.wua_busy;
                    let button = egui::Button::new("Perbarui driver");
                    let button = if enabled {
                        button.fill(theme::GREEN)
                    } else {
                        button
                    };
                    if ui.add_enabled(enabled, button).clicked() {
                        self.download_only = false;
                        self.install_selection = vec![update.id.clone()];
                        self.install_ack = false;
                        self.install_confirm = true;
                    }
                }
            } else {
                ui.label(
                    "Belum diperiksa. Pilih Periksa pembaruan driver untuk mencari secara online.",
                );
            }
            let eligible: Vec<_> = self
                .updates
                .as_ref()
                .map(|s| {
                    s.updates
                        .iter()
                        .filter(|u| u.eligible)
                        .map(|u| u.id.clone())
                        .collect()
                })
                .unwrap_or_default();
            let enabled = !eligible.is_empty()
                && rigpulse_drivers::privilege::is_admin()
                && !self.admin_busy
                && !self.install_busy
                && !self.wua_busy;
            let button = egui::Button::new(if enabled {
                RichText::new("Perbarui semua").color(theme::GREEN_TEXT)
            } else {
                RichText::new("Perbarui semua")
            });
            let button = if enabled {
                button.fill(theme::GREEN)
            } else {
                button
            };
            if ui.add_enabled(enabled,button).on_disabled_hover_text("Memerlukan paket terverifikasi dan dukungan instalasi langsung yang sudah divalidasi.").clicked(){self.download_only=false;self.install_selection=eligible;self.install_ack=false;self.install_confirm=true;}
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !self.install_selection.is_empty()
                            && !self.install_busy
                            && !self.wua_busy
                            && !self.admin_busy,
                        egui::Button::new("Unduh yang dipilih…"),
                    )
                    .clicked()
                {
                    self.download_only = true;
                    self.install_ack = false;
                    self.install_confirm = true;
                }
                if ui
                    .add_enabled(
                        !self.install_selection.is_empty()
                            && !self.install_busy
                            && !self.wua_busy
                            && !self.admin_busy
                            && rigpulse_drivers::privilege::is_admin(),
                        egui::Button::new("Pasang yang dipilih…"),
                    )
                    .clicked()
                {
                    self.download_only = false;
                    self.install_ack = false;
                    self.install_confirm = true;
                }
            });
            ui.label(RichText::new("Unduhan dan pemasangan menggunakan Windows Update Agent secara langsung. Izin administrator diperlukan untuk pemasangan; kebijakan organisasi tetap berlaku.").small().color(theme::MUTED));
            if self.install_busy {
                ui.spinner();
            }
            if let Some(state) = self.install_status {
                ui.label(format!("Operasi: {}", state_label(state)));
            }
            if self.install_busy {
                if let Some(p) = &self.driver_progress {
                    if let Some(percent) = p.percent {
                        ui.add(
                            egui::ProgressBar::new(percent as f32 / 100.0)
                                .text(format!("{percent}%")),
                        );
                    } else {
                        ui.label("Windows belum menyediakan persentase pada tahap ini.");
                    }
                    if let (Some(bytes), Some(total)) = (p.downloaded, p.total) {
                        ui.label(format!(
                            "{:.1} / {:.1} MB",
                            bytes / 1_000_000.0,
                            total / 1_000_000.0
                        ));
                    }
                    ui.label(format!(
                        "Tahap ini {:.0} detik · {}",
                        p.elapsed,
                        p.eta_seconds
                            .map(|s| format!("perkiraan sisa {:.0} detik", s))
                            .unwrap_or_else(|| "Perkiraan waktu belum tersedia".into())
                    ));
                    if p.abort_requested {
                        ui.label("Pembatalan diminta. Menunggu Windows menyelesaikan atau menghentikan langkah yang sedang berlangsung; paket yang telah terpasang tetap terpasang.");
                    }
                }
                if ui.button("Minta pembatalan").clicked() {
                    self.install_cancel
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
            if let Some(result) = &self.install_result {
                ui.label(&result.verification_note);
                if result.state == rigpulse_drivers::install::InstallState::RestartRequired {
                    ui.label(
                        "Windows meminta restart. Simpan pekerjaan dan restart saat Anda siap.",
                    );
                    ui.horizontal_wrapped(|ui|{if ui.button("Restart nanti").clicked(){self.notifications.push(crate::notifications::Kind::Information,"Restart ditunda. Anda dapat memulai ulang Windows setelah menyimpan pekerjaan.");}
if ui.button("Restart sekarang…").clicked(){self.restart_confirm=true;}});
                }
                egui::CollapsingHeader::new("Hasil teknis instalasi").show(ui, |ui| {
                    for p in &result.packages {
                        ui.label(format!(
                            "{} · hasil {}; HRESULT 0x{:08X}; restart {}",
                            p.update_id, p.result_code, p.hresult, p.restart_required
                        ));
                    }
                });
            }
        });
    }
}
pub(crate) fn state_label(state: rigpulse_drivers::install::InstallState) -> &'static str {
    use rigpulse_drivers::install::InstallState::*;
    match state {
        AwaitingConfirmation => "Memvalidasi paket resmi",
        Downloading => "Mengunduh",
        Installing => "Memasang driver",
        Verifying => "Memeriksa status perangkat",
        Completed => "Selesai",
        Downloaded => "Unduhan selesai",
        RestartRequired => "Windows meminta restart",
        PartiallyCompleted => "Selesai sebagian / perlu diperiksa",
        Cancelled => "Dibatalkan",
        Failed => "Gagal",
    }
}
fn filter_label(filter: usize) -> &'static str {
    match filter {
        1 => "Berfungsi",
        2 => "Perlu perhatian",
        3 => "Dinonaktifkan",
        4 => "Driver belum ada",
        5 => "Belum diketahui",
        _ => "Semua status",
    }
}
fn status_matches(status: &DeviceStatus, filter: usize) -> bool {
    match filter {
        1 => *status == DeviceStatus::Working,
        2 => attention(status),
        3 => *status == DeviceStatus::Disabled,
        4 => *status == DeviceStatus::MissingDriver,
        5 => *status == DeviceStatus::Unknown,
        _ => true,
    }
}

/// Absolute row/cell geometry avoids horizontal label centering and cumulative row drift.
fn cell(ui: &mut egui::Ui, rect: egui::Rect, text: RichText, tooltip: &str) {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("driver_cell", rect.min.x.to_bits(), rect.min.y.to_bits()))
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    child
        .add(egui::Label::new(text).truncate())
        .on_hover_text(tooltip);
}
fn button(ui: &mut egui::Ui, rect: egui::Rect, index: usize, selected: &mut Option<usize>) {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("driver_detail", index))
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::LEFT)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    if child
        .add_sized(rect.size(), egui::Button::new("Detail"))
        .clicked()
    {
        *selected = Some(index);
    }
}
fn table_columns(rect: egui::Rect) -> [egui::Rect; 5] {
    let width = rect.width();
    let action = 76.0;
    let status = 138.0;
    let version = 154.0;
    let category = 108.0;
    let gap = 12.0;
    let name = (width - action - status - version - category - gap * 4.0 - 24.0).max(160.0);
    let widths = [name, category, version, status, action];
    let mut x = rect.left() + 12.0;
    std::array::from_fn(|n| {
        let r = egui::Rect::from_min_size(
            egui::pos2(x, rect.top()),
            egui::vec2(widths[n], rect.height()),
        );
        x += widths[n] + gap;
        r
    })
}
fn render_table(
    ui: &mut egui::Ui,
    devices: &[DeviceDriverInfo],
    indices: &[usize],
    selected: &mut Option<usize>,
) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let width = ui.available_width();
        let wide = width >= 820.0;
        let height = if wide { 56.0 } else { 92.0 };
        if wide {
            let (header, _) = ui.allocate_exact_size(egui::vec2(width, 40.0), egui::Sense::hover());
            ui.painter().rect_filled(header, 6, theme::SIDEBAR);
            for (rect, label) in table_columns(header).into_iter().zip([
                "Nama perangkat",
                "Kategori",
                "Versi driver",
                "Status",
                "Aksi",
            ]) {
                cell(
                    ui,
                    rect,
                    RichText::new(label).small().strong().color(theme::MUTED),
                    label,
                );
            }
        }
        let top = ui.cursor().top();
        let clip = ui.clip_rect();
        let (first, last) = visible_rows(indices.len(), height, top, clip.top(), clip.bottom());
        if first > 0 {
            ui.allocate_exact_size(
                egui::vec2(width, first as f32 * height),
                egui::Sense::hover(),
            );
        }
        for (row, &index) in indices.iter().enumerate().take(last).skip(first) {
            let device = &devices[index];
            let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
            if row % 2 == 0 {
                ui.painter()
                    .rect_filled(rect, 0, theme::PANEL.gamma_multiply(0.55));
            }
            let color = if attention(&device.status) {
                theme::WARNING
            } else {
                theme::MUTED
            };
            if wide {
                let columns = table_columns(rect);
                for (cell_rect, text, primary) in [
                    (columns[0], device.name.as_str(), true),
                    (columns[1], category(&device.class), false),
                    (
                        columns[2],
                        device.version.as_deref().unwrap_or("Tidak tersedia"),
                        false,
                    ),
                    (columns[3], status(&device.status), false),
                ] {
                    cell(
                        ui,
                        cell_rect,
                        if primary {
                            RichText::new(text).strong()
                        } else {
                            RichText::new(text).color(if cell_rect == columns[3] {
                                color
                            } else {
                                theme::MUTED
                            })
                        },
                        text,
                    );
                }
                button(
                    ui,
                    egui::Rect::from_center_size(columns[4].center(), egui::vec2(76.0, 34.0)),
                    index,
                    selected,
                );
            } else {
                let left = rect.left() + 12.0;
                let text_width = (width - 112.0).max(120.0);
                let line = |y| {
                    egui::Rect::from_min_size(
                        egui::pos2(left, rect.top() + y),
                        egui::vec2(text_width, 24.0),
                    )
                };
                cell(
                    ui,
                    line(9.0),
                    RichText::new(&device.name).strong(),
                    &device.name,
                );
                let metadata = format!(
                    "{} · Versi {}",
                    category(&device.class),
                    device.version.as_deref().unwrap_or("Tidak tersedia")
                );
                cell(
                    ui,
                    line(33.0),
                    RichText::new(&metadata).small().color(theme::MUTED),
                    &metadata,
                );
                cell(
                    ui,
                    line(57.0),
                    RichText::new(status(&device.status)).small().color(color),
                    status(&device.status),
                );
                button(
                    ui,
                    egui::Rect::from_min_size(
                        egui::pos2(rect.right() - 88.0, rect.center().y - 17.0),
                        egui::vec2(76.0, 34.0),
                    ),
                    index,
                    selected,
                );
            }
            ui.painter().hline(
                rect.x_range(),
                rect.bottom(),
                egui::Stroke::new(1.0, theme::BORDER),
            );
        }
        if last < indices.len() {
            ui.allocate_exact_size(
                egui::vec2(width, (indices.len() - last) as f32 * height),
                egui::Sense::hover(),
            );
        }
    });
}
fn attention(status: &DeviceStatus) -> bool {
    matches!(
        status,
        DeviceStatus::Issue | DeviceStatus::MissingDriver | DeviceStatus::Disabled
    )
}
fn status(status: &DeviceStatus) -> &'static str {
    match status {
        DeviceStatus::Working => "Berfungsi",
        DeviceStatus::Issue => "Perlu perhatian",
        DeviceStatus::MissingDriver => "Driver belum ada",
        DeviceStatus::Disabled => "Dinonaktifkan",
        DeviceStatus::Unknown => "Belum diketahui",
    }
}
fn category(class: &str) -> &'static str {
    match class.to_ascii_lowercase().as_str() {
        "display" | "monitor" => "Grafis",
        "media" | "audioendpoint" => "Audio",
        "net" => "Jaringan",
        "bluetooth" => "Bluetooth",
        "diskdrive" | "hdc" | "scsiadapter" | "volume" => "Penyimpanan",
        "system" | "processor" | "computer" | "firmware" => "Sistem",
        _ => "Lainnya",
    }
}
fn visible_rows(
    count: usize,
    height: f32,
    top: f32,
    clip_top: f32,
    clip_bottom: f32,
) -> (usize, usize) {
    let first = (((clip_top - top) / height).floor().max(0.0) as usize).min(count);
    let last = ((((clip_bottom - top) / height).ceil().max(0.0) as usize) + 1)
        .min(count)
        .max(first);
    (first, last)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_filters_follow_query_category_status_and_replaced_inventory() {
        let mut devices = vec![DeviceDriverInfo {
            name: "NVIDIA GeForce RTX".into(),
            class: "Display".into(),
            manufacturer: Some("NVIDIA".into()),
            provider: None,
            version: Some("32.0".into()),
            date: None,
            status: DeviceStatus::Working,
            problem: None,
            instance_id: "gpu".into(),
            hardware_ids: vec![],
        }];
        let mut view = DriverView::default();
        view.refresh(0, &devices, "rtx", "Grafis", 1);
        assert_eq!(view.indices, vec![0]);
        view.refresh(0, &devices, "rtx", "Jaringan", 1);
        assert!(view.indices.is_empty());
        view.refresh(0, &devices, "missing", "", 0);
        assert!(view.indices.is_empty());
        devices[0].status = DeviceStatus::MissingDriver;
        view.refresh(1, &devices, "32.0", "", 4);
        assert_eq!(view.indices, vec![0]);
        assert_eq!(view.issues, 1);
        view.refresh(1, &devices, "", "", 1);
        assert!(view.indices.is_empty());
    }
    #[test]
    fn large_list_only_renders_viewport() {
        let (first, last) = visible_rows(500, 60.0, 0.0, 6000.0, 6600.0);
        assert_eq!(first, 100);
        assert!(last - first <= 12);
        assert_eq!(visible_rows(0, 60.0, 0.0, 0.0, 700.0), (0, 0));
    }
    #[test]
    fn categories_and_attention() {
        assert_eq!(category("Display"), "Grafis");
        assert_eq!(category("Net"), "Jaringan");
        assert!(attention(&DeviceStatus::MissingDriver));
        assert!(!attention(&DeviceStatus::Unknown));
    }
    #[test]
    fn driver_columns_and_virtual_rows_preserve_geometry() {
        for width in [320.0, 520.0, 840.0, 1200.0] {
            let data: Vec<_> = (0..500)
                .map(|n| DeviceDriverInfo {
                    name: format!("Test device with a long model name {n}"),
                    class: "Display".into(),
                    manufacturer: None,
                    provider: None,
                    version: Some("10.20.30.40".into()),
                    date: None,
                    status: DeviceStatus::Working,
                    problem: Some(0),
                    instance_id: String::new(),
                    hardware_ids: Vec::new(),
                })
                .collect();
            let rows: Vec<_> = (0..data.len()).collect();
            let ctx = egui::Context::default();
            theme::install(&ctx);
            let mut total = 0.0;
            let mut available = 0.0;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 600.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        available = ui.available_width();
                        let top = ui.cursor().top();
                        render_table(ui, &data, &rows, &mut None);
                        total = ui.cursor().top() - top;
                    });
                },
            );
            let expected = if available >= 820.0 {
                500.0 * 56.0 + 40.0
            } else {
                500.0 * 92.0
            } + ctx.style().spacing.item_spacing.y;
            assert!(
                (total - expected).abs() < 1.0,
                "Rows drifted at {width}: {total} versus {expected}"
            );
            if available >= 820.0 {
                let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(available, 56.0));
                let columns = table_columns(rect);
                assert!(columns[4].right() <= rect.right());
                for pair in columns.windows(2) {
                    assert!(pair[0].right() < pair[1].left());
                }
            }
        }
        assert!(status_matches(&DeviceStatus::Disabled, 3));
        assert!(!status_matches(&DeviceStatus::Working, 3));
    }
}
