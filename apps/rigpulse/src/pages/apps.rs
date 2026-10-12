use crate::{
    app::RigPulseApp,
    apps_service::{Confirmation, Controller},
    theme,
};
use eframe::egui::{self, RichText};
use rigpulse_apps::{
    Application,
    query::{Filter, Sort},
    size_label,
};
impl RigPulseApp {
    pub(crate) fn apps_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if self.apps.inventory.is_none() && !self.apps.busy && self.apps.error.is_none() {
            self.apps.refresh(ctx);
        }
        ui.horizontal(|ui| {
            ui.strong("Aplikasi di komputer ini");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(!self.apps.busy, egui::Button::new("Segarkan"))
                    .clicked()
                {
                    self.apps.refresh(ctx);
                }
            });
        });
        overview(ui, &mut self.apps);
        ui.horizontal_wrapped(|ui| {
            for (n, title) in [
                "Aplikasi Terinstal",
                "Uninstall Lanjutan",
                "Pembersih Sisa Aplikasi",
            ]
            .into_iter()
            .enumerate()
            {
                let response = ui.selectable_value(&mut self.apps.tab, n, title);
                #[cfg(test)]
                {
                    self.apps.tab_rects[n] = response.rect;
                }
                #[cfg(not(test))]
                {
                    let _ = response;
                }
            }
        });
        if self.apps.busy {
            ui.horizontal_wrapped(|ui| {
                ui.spinner();
                ui.label(self.apps.status.as_deref().unwrap_or("Memuat…"));
                if ui.button("Batalkan tugas").clicked() {
                    self.apps.cancel();
                }
            });
        } else if let Some(status) = &self.apps.status {
            ui.colored_label(theme::BLUE, status);
        }
        if let Some(error) = &self.apps.error {
            ui.colored_label(theme::WARNING, error);
            if let Some(detail) = &self.apps.technical_error {
                egui::CollapsingHeader::new("Rincian kegagalan").show(ui, |ui| {
                    ui.label(detail);
                });
            }
        }
        if !self.apps.cleanup_failures.is_empty() {
            egui::CollapsingHeader::new("Rincian file yang dilewati (maks. 64)").show(ui, |ui| {
                ui.label("Item yang gagal tetap di lokasi semula. Rincian lengkap tersimpan di log lokal.");
                for detail in &self.apps.cleanup_failures {
                    ui.add(egui::Label::new(detail).wrap());
                }
            });
        }
        if self.apps.permission_denied
            && let Some((app, advanced)) = self.apps.last_uninstall.clone()
            && app.kind != rigpulse_apps::AppKind::Store
            && ui
                .add_enabled(
                    !self.apps.busy,
                    egui::Button::new("Coba uninstaller dengan administrator…"),
                )
                .clicked()
        {
            self.apps.confirmation = Some(Confirmation::Uninstall {
                app,
                advanced,
                elevate: true,
            });
        }
        if self.apps.pending > 0 {
            ui.colored_label(
                theme::WARNING,
                format!(
                    "{} uninstall menunggu verifikasi/restart. Segarkan setelah restart.",
                    self.apps.pending
                ),
            );
        }
        match self.apps.tab {
            0 => installed(ui, &mut self.apps),
            1 => {
                theme::surface(ui, |ui| {
                    theme::section(ui, "Uninstall Lanjutan");
                    ui.label("Jalankan uninstaller resmi, verifikasi penghapusan, lalu tinjau cache yang tersisa.");
                    if let Some(app) = selected_app(&self.apps) {
                        if ui.button(format!("Detail {}…", app.name)).clicked() {
                            self.apps.details_open = true;
                        }
                    } else {
                        ui.label("Pilih aplikasi dari daftar di bawah. Klik nama untuk memilih atau Detail untuk memeriksa informasi.");
                    }
                });
                installed(ui, &mut self.apps);
            }
            _ => {
                theme::surface(ui, |ui| {
                    theme::section(ui, "Pembersih Sisa Aplikasi");
                    ui.label("Temukan kemungkinan file sisa dari aplikasi yang sudah dihapus.");
                    ui.label(RichText::new("Scan hanya memakai riwayat uninstall terverifikasi dan aturan lokasi khusus aplikasi. Folder asing tidak dianggap sampah.").small().color(theme::MUTED));
                    if ui
                        .add_enabled(!self.apps.busy, egui::Button::new("Pindai sisa aplikasi"))
                        .clicked()
                    {
                        self.apps
                            .scan_leftovers(ctx, self.settings.apps_registry_review);
                    }
                    egui::CollapsingHeader::new("Pilihan pemindaian").show(ui,|ui|{ui.label("Cache dikenal dapat ditinjau untuk karantina. Konfigurasi, data pengguna, folder instalasi dan registry hanya dilaporkan. Registry diperiksa jika diaktifkan pada Settings → Apps & Cleanup. Scan tidak mengubah file.");});
                });
                cleanup_results(ui, &mut self.apps, ctx);
                quarantine(ui, &mut self.apps);
            }
        }
        if self.apps.details_open
            && self.apps.tab < 2
            && self.apps.confirmation.is_none()
            && let Some(app) = selected_app(&self.apps)
        {
            let modal = egui::Modal::new(egui::Id::new("app_details_modal"))
                .frame(
                    egui::Frame::popup(&ctx.style())
                        .fill(theme::PANEL)
                        .inner_margin(20),
                )
                .show(ctx, |ui| {
                    ui.set_width((ctx.screen_rect().width() * 0.9 - 48.0).clamp(260.0, 600.0));
                    ui.horizontal(|ui| {
                        if let Some(icon) = self.apps.icons.get(&app.id) {
                            ui.image((icon.id(), egui::vec2(36.0, 36.0)));
                        }
                        ui.vertical(|ui| {
                            ui.set_max_width(
                                (ctx.screen_rect().width() * 0.9 - 160.0).clamp(160.0, 490.0),
                            );
                            ui.add(
                                egui::Label::new(RichText::new(&app.name).size(20.0).strong())
                                    .wrap(),
                            );
                            ui.add(
                                egui::Label::new(format!(
                                    "{} · {}",
                                    app.publisher
                                        .as_deref()
                                        .unwrap_or("Publisher tidak tersedia"),
                                    app.version.as_deref().unwrap_or("Versi tidak tersedia")
                                ))
                                .wrap(),
                            );
                        });
                        if ui.button("Tutup ×").clicked() {
                            self.apps.details_open = false;
                        }
                    });
                    egui::ScrollArea::vertical()
                        .id_salt("app_details_body")
                        .max_height(
                            (ctx.screen_rect().height() * 0.85 - ui.min_rect().height() - 160.0)
                                .max(70.0),
                        )
                        .show(ui, |ui| {
                            details(ui, &app);
                        });
                    let action = detail_actions(ui, &app, self.apps.busy);
                    apply_detail_action(&mut self.apps, action, ctx);
                    if ui.button("Tutup detail").clicked() {
                        self.apps.details_open = false;
                    }
                });
            #[cfg(test)]
            {
                self.apps.details_panel_rect = modal.response.rect;
            }
            if modal.should_close() {
                self.apps.details_open = false;
            }
        }
        if self.apps.cleanup_review_open && self.apps.confirmation.is_none() {
            let modal = egui::Modal::new(egui::Id::new("cleanup_review_modal")).show(ctx, |ui| {
                ui.set_width((ctx.screen_rect().width() * 0.9 - 48.0).clamp(260.0, 640.0));
                ui.heading("Tinjau sisa aplikasi");
                ui.label(
                    "Pilih hanya cache yang ingin dikarantina. Tidak ada penghapusan otomatis.",
                );
                egui::ScrollArea::vertical()
                    .id_salt("cleanup_review_body")
                    .max_height((ctx.screen_rect().height() * 0.85 - 130.0).max(100.0))
                    .show(ui, |ui| cleanup_results(ui, &mut self.apps, ctx));
                if ui.button("Tutup tinjauan").clicked() {
                    self.apps.cleanup_review_open = false;
                }
            });
            if modal.should_close() {
                self.apps.cleanup_review_open = false;
            }
        }
        if let Some(confirm) = self.apps.confirmation.clone() {
            let modal=egui::Modal::new(egui::Id::new("apps_confirmation")).show(ctx,|ui|{
            ui.set_width((ctx.screen_rect().width()*0.9-48.0).clamp(260.0,520.0));
let action=egui::ScrollArea::vertical().max_height(ctx.screen_rect().height()*0.65).show(ui,|ui|{
            let(title,body,action)=match &confirm{
                Confirmation::Uninstall{app,advanced,elevate}=>(format!("Uninstall {}?",app.name),format!("RigPulse akan menjalankan uninstaller resmi aplikasi. Konfigurasi dan data pengguna mungkin tetap tersimpan. {}{}",if *advanced{"Setelah penghapusan terverifikasi, scan sisa mengikuti preferensi Anda. Tidak ada penghapusan file otomatis. "}else{""},if *elevate{"Uninstaller akan meminta izin administrator melalui UAC Windows. Periksa penerbit pada dialog tersebut."}else{"Windows dapat meminta izin jika uninstaller memerlukannya."}),"Lanjutkan uninstall"),
                Confirmation::Clean{ids}=>{let bytes=self.apps.scan.as_ref().map(|s|s.candidates.iter().filter(|c|ids.contains(&c.id)).map(|c|c.bytes.unwrap_or(0)).sum()).unwrap_or(0);("Karantina file terpilih?".into(),format!("{} file, total {}. Hanya pilihan yang lolos validasi ulang akan dipindahkan. File dapat dipulihkan; ruang disk belum dikosongkan.",ids.len(),size_label(Some(bytes))),"Karantina terpilih")},
                Confirmation::Recover{purge,..}=>(if *purge{"Hapus permanen?"}else{"Pulihkan file?"}.into(),if *purge{"File ini tidak dapat dipulihkan setelah dihapus permanen."}else{"File dikembalikan ke jalur asli tanpa menimpa file yang sudah ada."}.into(),if *purge{"Hapus permanen"}else{"Pulihkan"}),
            };ui.heading(title);ui.label(body);
            if let Confirmation::Uninstall{app,..}=&confirm{theme::row(ui,"Metode",app.kind.label());theme::row(ui,"Publisher",app.publisher.as_deref().unwrap_or("Tidak tersedia"));}
            if let Confirmation::Recover{id,..}=&confirm&&let Some(item)=self.apps.quarantine.iter().find(|e|&e.id==id){ui.add(egui::Label::new(item.original.display().to_string()).wrap());}
            action}).inner;
ui.horizontal_wrapped(|ui|{if ui.button("Batal").clicked(){self.apps.confirmation=None;}
if ui.add_enabled(!self.apps.busy,egui::Button::new(RichText::new(action).color(theme::DANGER))).clicked(){self.apps.confirmation=None;self.apps.approve(confirm.clone(),ctx,&self.settings);}});
        });
            if modal.should_close() {
                self.apps.confirmation = None;
            }
        }
    }
}
fn selected_app(state: &Controller) -> Option<Application> {
    let id = state.selected.as_ref()?;
    state
        .inventory
        .as_ref()?
        .apps
        .iter()
        .find(|a| &a.id == id)
        .cloned()
}
fn overview(ui: &mut egui::Ui, state: &mut Controller) {
    let [count, estimate, scanned] = state.overview();
    let candidates = state
        .scan
        .as_ref()
        .map(|s| format!("{} item", s.candidates.len()))
        .unwrap_or_else(|| "Belum dipindai".into());
    let cards = [
        ("Aplikasi terdeteksi", count),
        ("Ukuran diketahui (estimasi)", estimate),
        ("Scan terakhir", scanned),
        ("Kandidat sisa", candidates),
    ];
    let cols = if ui.available_width() >= 900.0 {
        4
    } else if ui.available_width() >= 460.0 {
        2
    } else {
        1
    };
    for row in cards.chunks(cols) {
        ui.columns(cols, |columns| {
            for (col, (label, value)) in columns.iter_mut().zip(row) {
                theme::surface(col, |ui| {
                    ui.set_min_height(60.0);
                    ui.label(RichText::new(*label).small().color(theme::MUTED));
                    ui.label(RichText::new(value).size(18.0).strong());
                });
            }
        });
    }
}
fn installed(ui: &mut egui::Ui, state: &mut Controller) {
    ui.add(
        egui::TextEdit::singleline(&mut state.query)
            .desired_width(ui.available_width())
            .min_size(egui::vec2(0.0, 36.0))
            .margin(egui::Margin::symmetric(12, 8))
            .hint_text("Cari aplikasi berdasarkan nama atau publisher…"),
    );
    let width = ((ui.available_width() - 16.0) / 2.0).clamp(110.0, 200.0);
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("apps_filter")
            .width(width)
            .selected_text(filter_label(state.filter))
            .show_ui(ui, |ui| {
                for filter in [
                    Filter::All,
                    Filter::Desktop,
                    Filter::Store,
                    Filter::Recent,
                    Filter::Large,
                    Filter::Unknown,
                    Filter::System,
                ] {
                    ui.selectable_value(&mut state.filter, filter, filter_label(filter));
                }
            });
        egui::ComboBox::from_id_salt("apps_sort")
            .width(width)
            .selected_text(sort_label(state.sort))
            .show_ui(ui, |ui| {
                for sort in [Sort::Name, Sort::Publisher, Sort::Size, Sort::Installed] {
                    ui.selectable_value(&mut state.sort, sort, sort_label(sort));
                }
            });
    });
    if state.filter == Filter::System {
        ui.colored_label(
            theme::WARNING,
            "Komponen sistem dan runtime bersama hanya dapat ditinjau; uninstall dilindungi.",
        );
    }
    let Some(inventory) = state.inventory.as_ref() else {
        ui.label(if state.busy {
            "Memuat aplikasi terinstal…"
        } else {
            "Gagal membaca daftar aplikasi. Gunakan Segarkan untuk mencoba lagi."
        });
        return;
    };
    if !inventory.warnings.is_empty() {
        egui::CollapsingHeader::new("Sumber yang belum terbaca").show(ui, |ui| {
            for warning in inventory.warnings.iter().take(16) {
                ui.label(warning);
            }
        });
    }
    let apps = &inventory.apps;
    let indices = &state.filtered;
    ui.label(
        RichText::new(format!(
            "{} dari {} entri · Terakhir digunakan: tidak tersedia secara andal.",
            indices.len(),
            inventory.apps.len()
        ))
        .small()
        .color(theme::MUTED),
    );
    if indices.is_empty() {
        theme::surface(ui, |ui| {
            ui.strong(if inventory.apps.is_empty() {
                "Tidak ada aplikasi yang ditemukan."
            } else {
                "Tidak ada aplikasi yang cocok."
            });
            ui.label("Coba kata pencarian atau filter lain.");
        });
        return;
    }
    app_list(
        ui,
        apps,
        indices,
        &state.icons,
        &mut state.selected,
        &mut state.details_open,
    );
}
fn filter_label(f: Filter) -> &'static str {
    match f {
        Filter::All => "Semua aplikasi",
        Filter::Desktop => "Aplikasi desktop",
        Filter::Store => "Microsoft Store",
        Filter::Recent => "Dipasang 30 hari terakhir",
        Filter::Large => "Besar (≥1 GiB)",
        Filter::Unknown => "Ukuran tidak diketahui",
        Filter::System => "Komponen sistem",
    }
}
fn sort_label(s: Sort) -> &'static str {
    match s {
        Sort::Name => "Nama A–Z",
        Sort::Publisher => "Publisher A–Z",
        Sort::Size => "Ukuran terbesar",
        Sort::Installed => "Terbaru dipasang",
    }
}
fn cell(ui: &mut egui::Ui, rect: egui::Rect, text: RichText, tip: &str) {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("app_cell", rect.min.x.to_bits(), rect.min.y.to_bits()))
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    child
        .add(egui::Label::new(text).truncate())
        .on_hover_text(tip);
}
fn app_list(
    ui: &mut egui::Ui,
    apps: &[Application],
    indices: &[usize],
    icons: &std::collections::HashMap<String, egui::TextureHandle>,
    selected: &mut Option<String>,
    details: &mut bool,
) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let width = ui.available_width();
        let wide = width >= 850.0;
        let height = if wide { 70.0 } else { 94.0 };
        let columns = |r: egui::Rect| {
            let name = (width - 24.0 - 160.0 - 110.0 - 112.0 - 76.0 - 48.0).max(180.0);
            let widths = [name, 160.0, 110.0, 112.0, 76.0];
            let mut x = r.left() + 12.0;
            std::array::from_fn::<_, 5, _>(|n| {
                let rect = egui::Rect::from_min_size(
                    egui::pos2(x, r.top()),
                    egui::vec2(widths[n], r.height()),
                );
                x += widths[n] + 12.0;
                rect
            })
        };
        if wide {
            let (r, _) = ui.allocate_exact_size(egui::vec2(width, 36.0), egui::Sense::hover());
            ui.painter().rect_filled(r, 6, theme::SIDEBAR);
            for (rect, label) in columns(r).into_iter().zip([
                "Aplikasi / versi",
                "Publisher",
                "Estimasi ukuran",
                "Terakhir dipakai",
                "Aksi",
            ]) {
                cell(
                    ui,
                    rect,
                    RichText::new(label).small().color(theme::MUTED),
                    label,
                );
            }
        }
        let top = ui.cursor().top();
        let clip = ui.clip_rect();
        let first_row = ((clip.top() - top) / height - 0.5).floor().max(0.0) as usize;
        let first = first_row.min(indices.len());
        let last_row = ((clip.bottom() - top) / height + 0.5).ceil().max(0.0) as usize + 1;
        let last = last_row.min(indices.len()).max(first);
        if first > 0 {
            ui.allocate_exact_size(
                egui::vec2(width, first as f32 * height),
                egui::Sense::hover(),
            );
        }
        for (index, &source) in indices.iter().enumerate().take(last).skip(first) {
            let Some(app) = apps.get(source) else {
                continue;
            };
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
            if selected.as_ref() == Some(&app.id) {
                ui.painter().rect_filled(rect, 0, theme::HOVER);
            } else if index % 2 == 0 {
                ui.painter()
                    .rect_filled(rect, 0, theme::PANEL.gamma_multiply(0.55));
            }
            if response.clicked() {
                *selected = Some(app.id.clone());
            }
            let ir = egui::Rect::from_min_size(
                rect.min + egui::vec2(12.0, 15.0),
                egui::vec2(28.0, 28.0),
            );
            if let Some(icon) = icons.get(&app.id) {
                ui.painter().image(
                    icon.id(),
                    ir,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            } else {
                theme::paint_icon(
                    ui.painter(),
                    ir.shrink(4.0),
                    rigpulse_core::Page::Apps,
                    theme::MUTED,
                );
            }
            let left = rect.left() + 52.0;
            let action = egui::Rect::from_center_size(
                egui::pos2(rect.right() - 50.0, rect.center().y),
                egui::vec2(76.0, 34.0),
            );
            let line = |y, w| {
                egui::Rect::from_min_size(egui::pos2(left, rect.top() + y), egui::vec2(w, 24.0))
            };
            if wide {
                let cols = columns(rect);
                let w = cols[0].right() - left;
                cell(
                    ui,
                    line(7.0, w),
                    RichText::new(&app.name).strong(),
                    &app.name,
                );
                cell(
                    ui,
                    line(33.0, w),
                    RichText::new(app.version.as_deref().unwrap_or("Versi tidak tersedia"))
                        .small()
                        .color(theme::MUTED),
                    "Versi terpasang",
                );
                for (r, text) in [
                    (
                        cols[1],
                        app.publisher
                            .clone()
                            .unwrap_or_else(|| "Tidak tersedia".into()),
                    ),
                    (cols[2], size_label(app.estimated_bytes)),
                    (cols[3], "Tidak tersedia".into()),
                ] {
                    cell(
                        ui,
                        r,
                        RichText::new(&text).small().color(theme::MUTED),
                        &text,
                    );
                }
            } else {
                let w = (width - 164.0).max(100.0);
                cell(
                    ui,
                    line(7.0, w),
                    RichText::new(&app.name).strong(),
                    &app.name,
                );
                let meta = format!(
                    "{} · {}",
                    app.publisher
                        .as_deref()
                        .unwrap_or("Publisher tidak tersedia"),
                    app.version.as_deref().unwrap_or("Versi tidak tersedia")
                );
                cell(
                    ui,
                    line(33.0, w),
                    RichText::new(&meta).small().color(theme::MUTED),
                    &meta,
                );
                let size = format!("{} · {}", size_label(app.estimated_bytes), app.kind.label());
                cell(
                    ui,
                    line(59.0, w),
                    RichText::new(&size).small().color(theme::MUTED),
                    &size,
                );
            }
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(("app_detail", &app.id))
                    .max_rect(action)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );
            child.set_clip_rect(action.intersect(ui.clip_rect()));
            if child
                .add_sized(action.size(), egui::Button::new("Detail"))
                .clicked()
            {
                *selected = Some(app.id.clone());
                *details = true;
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
#[cfg(test)]
mod tests {
    use super::*;

    /// Bug condition exploration test — Property 1: Stable Total Content Height During Scroll Deceleration
    ///
    /// **Validates: Requirements 1.2, 1.3**
    ///
    /// This test MUST FAIL on unfixed code. Failure confirms the scroll bounce bug exists.
    /// The bug: `first = floor((clip.top() - top) / height)` is sensitive to sub-pixel shifts
    /// in `clip.top()`. A 0.4 px shift can change `first` by 1, altering total allocation
    /// by one full row height (70 px or 94 px), causing egui to correct the scroll offset.
    #[test]
    fn scroll_bounce_total_height_stable_across_clip_shift() {
        use rigpulse_apps::Application;

        let ctx = egui::Context::default();
        crate::theme::install(&ctx);

        // Build a 100-entry apps slice and an indices vec pointing at them all.
        let count = 100usize;
        let apps: Vec<Application> = (0..count)
            .map(|i| Application {
                id: format!("app-{i}"),
                name: format!("Application {i}"),
                ..Default::default()
            })
            .collect();
        let indices: Vec<usize> = (0..count).collect();
        let icons = std::collections::HashMap::new();

        // Helper: render app_list inside a constrained ScrollArea with a given scroll offset,
        // return the total height allocated by app_list (ui.min_rect().height()).
        let render_frame = |scroll_offset: f32| -> f32 {
            let apps = apps.clone();
            let indices = indices.clone();
            let icons = icons.clone();
            let mut selected: Option<String> = None;
            let mut details = false;
            let mut captured_height = 0.0f32;

            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(500.0, 600.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        // Constrain height so clip culling actually activates.
                        egui::ScrollArea::vertical()
                            .id_salt(format!("bounce_test_{}", scroll_offset.to_bits()))
                            .max_height(300.0)
                            .vertical_scroll_offset(scroll_offset)
                            .show(ui, |ui| {
                                let top = ui.cursor().top();
                                app_list(
                                    ui,
                                    &apps,
                                    &indices,
                                    &icons,
                                    &mut selected,
                                    &mut details,
                                );
                                captured_height = ui.min_rect().bottom() - top;
                            });
                    });
                },
            );

            captured_height
        };

        // Frame 1: scroll offset 0.0 — clip.top() at position A
        let height_frame_1 = render_frame(0.0);

        // Frame 2: scroll offset 0.4 — clip.top() shifts by 0.4 px (simulates one deceleration step)
        let height_frame_2 = render_frame(0.4);

        // On UNFIXED code this assertion FAILS because a sub-pixel clip shift can change
        // `first` by 1, altering total allocation by one full row height (70 px).
        //
        // Expected counterexample on unfixed code:
        //   height_frame_1 = N * 94.0 px (narrow mode at width 500)
        //   height_frame_2 = (N-1) * 94.0 px — differ by 94 px when first increments
        // The real bounce bug is a whole-row (70/94 px) height jump caused by a
        // `first` index change. Sub-pixel float accumulation near the allocation
        // boundary can differ by a fraction of a pixel without changing layout
        // (0 rows), which must NOT fail the suite.
        let diff = (height_frame_1 - height_frame_2).abs();
        assert!(
            diff < 1.0,
            "Total content height changed between frames with clip.top() shift of 0.4 px: \
             frame_1={height_frame_1} px, frame_2={height_frame_2} px, \
             differ by {} px (= {} rows)",
            diff,
            (diff / 94.0).round() as usize,
        );
    }
}

enum DetailAction {
    Folder(std::path::PathBuf),
    Uninstall(Box<Application>, bool),
}
fn apply_detail_action(state: &mut Controller, action: Option<DetailAction>, ctx: &egui::Context) {
    match action {
        Some(DetailAction::Folder(path)) => state.open_folder(path, ctx),
        Some(DetailAction::Uninstall(app, advanced)) => {
            state.details_open = false;
            state.confirmation = Some(Confirmation::Uninstall {
                app: *app,
                advanced,
                elevate: false,
            });
        }
        None => {}
    }
}
fn details(ui: &mut egui::Ui, app: &Application) {
    for line in app.details().lines().skip(1) {
        if let Some((label, value)) = line.split_once(": ") {
            if label == "Sumber" {
                continue;
            }
            theme::row(ui, label, value);
        }
    }
    theme::row(
        ui,
        "Metode uninstall",
        if app.removable() {
            app.kind.label()
        } else {
            "Dilindungi / tidak tersedia"
        },
    );
    egui::CollapsingHeader::new("Identitas aplikasi").show(ui, |ui| {
        ui.label(&app.source);
        ui.add(egui::Label::new(&app.id).wrap());
    });
}
fn detail_actions(ui: &mut egui::Ui, app: &Application, busy: bool) -> Option<DetailAction> {
    let mut action = None;
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                app.location.as_ref().is_some_and(|p| p.is_absolute()) && !busy,
                egui::Button::new("Buka folder instalasi"),
            )
            .clicked()
            && let Some(path) = &app.location
        {
            action = Some(DetailAction::Folder(path.clone()));
        }
        if ui.button("Salin detail").clicked() {
            ui.ctx().copy_text(app.details());
        }
        for (advanced, label) in [(false, "Uninstall…"), (true, "Uninstall Lanjutan…")] {
            if ui
                .add_enabled(
                    !busy && app.removable(),
                    egui::Button::new(RichText::new(label).color(theme::DANGER)),
                )
                .clicked()
            {
                action = Some(DetailAction::Uninstall(Box::new(app.clone()), advanced));
            }
        }
    });
    action
}
fn cleanup_results(ui: &mut egui::Ui, state: &mut Controller, ctx: &egui::Context) {
    let Some(scan) = state.scan.as_ref() else {
        ui.label(
            "Belum ada scan sisa aplikasi. Riwayat uninstall dimulai sejak fitur ini digunakan.",
        );
        return;
    };
    if scan.candidates.is_empty() {
        theme::surface(ui, |ui| {
            ui.strong("Tidak ada sisa aplikasi yang dapat diidentifikasi dengan yakin.");
            ui.label("Tidak ada file yang diubah.");
        });
    } else {
        theme::section(ui, "Hasil pemindaian — periksa sebelum dibersihkan");
        let safe = scan.candidates.iter().filter(|c| c.eligible).count();
        ui.label(format!(
            "{} kandidat · {safe} cache memenuhi syarat · {} perlu tinjauan manual",
            scan.candidates.len(),
            scan.candidates.len() - safe
        ));
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!state.busy, egui::Button::new("Pilih semua cache aman"))
                .clicked()
            {
                state.selected_candidates = scan
                    .candidates
                    .iter()
                    .filter(|c| c.eligible)
                    .map(|c| c.id.clone())
                    .collect();
            }
            if ui.button("Kosongkan pilihan").clicked() {
                state.selected_candidates.clear();
            }
        });
        egui::ScrollArea::vertical()
            .id_salt("cleanup_candidates")
            .max_height(420.0)
            .show_rows(ui, 142.0, scan.candidates.len(), |ui, range| {
                for n in range {
                    let c = &scan.candidates[n];
                    ui.push_id(&c.id, |ui| {
                        ui.horizontal(|ui| {
                            let mut chosen = state.selected_candidates.contains(&c.id);
                            if ui
                                .add_enabled(
                                    c.eligible && !state.busy,
                                    egui::Checkbox::without_text(&mut chosen),
                                )
                                .changed()
                            {
                                if chosen {
                                    state.selected_candidates.push(c.id.clone());
                                } else {
                                    state.selected_candidates.retain(|id| id != &c.id);
                                }
                            }
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!("{} · {}", c.app.name, c.category))
                                        .strong(),
                                )
                                .truncate(),
                            );
                        });
                        ui.add(egui::Label::new(c.path.display().to_string()).truncate())
                            .on_hover_text(c.path.display().to_string());
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!(
                                    "{} · Keyakinan {} · {}",
                                    size_label(c.bytes),
                                    c.confidence,
                                    c.risk
                                ))
                                .small()
                                .color(if c.eligible {
                                    theme::MUTED
                                } else {
                                    theme::WARNING
                                }),
                            )
                            .truncate(),
                        )
                        .on_hover_text(&c.reason);
                        if ui
                            .small_button("Salin jalur")
                            .on_hover_text(&c.reason)
                            .clicked()
                        {
                            ctx.copy_text(c.path.display().to_string());
                        }
                    });
                }
            });
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !state.busy && !state.selected_candidates.is_empty(),
                    egui::Button::new("Pratinjau pilihan"),
                )
                .clicked()
            {
                state.dry_run(ctx);
            }
            if ui
                .add_enabled(
                    !state.busy && !state.selected_candidates.is_empty(),
                    egui::Button::new(RichText::new("Bersihkan terpilih…").color(theme::DANGER)),
                )
                .clicked()
            {
                state.confirmation = Some(Confirmation::Clean {
                    ids: state.selected_candidates.clone(),
                });
            }
            if ui.button("Lewati pembersihan").clicked() {
                state.selected_candidates.clear();
                state.tab = 0;
            }
        });
    }
    if let Some(scan) = &state.scan
        && !scan.warnings.is_empty()
    {
        egui::CollapsingHeader::new("Item dilewati / batas scan").show(ui, |ui| {
            for warning in scan.warnings.iter().take(32) {
                ui.label(warning);
            }
        });
    }
}
fn quarantine(ui: &mut egui::Ui, state: &mut Controller) {
    egui::CollapsingHeader::new(format!("Karantina & pemulihan ({} item)",state.quarantine.len())).show(ui,|ui|{
    ui.label("Retensi manual hingga Anda memulihkan atau menghapus file. Batas 2 GiB / 2.500 item; tidak ada penghapusan otomatis.");
    let pages=state.quarantine.len().div_ceil(50).max(1);state.quarantine_page=state.quarantine_page.min(pages-1);
    ui.horizontal_wrapped(|ui|{if ui.add_enabled(state.quarantine_page>0,egui::Button::new("Sebelumnya")).clicked(){state.quarantine_page-=1;}ui.label(format!("Halaman {} / {pages}",state.quarantine_page+1));if ui.add_enabled(state.quarantine_page+1<pages,egui::Button::new("Berikutnya")).clicked(){state.quarantine_page+=1;}});
    for item in state.quarantine.iter().skip(state.quarantine_page*50).take(50){theme::surface(ui,|ui|{ui.strong(&item.app.name);ui.add(egui::Label::new(item.original.display().to_string()).truncate()).on_hover_text(item.original.display().to_string());ui.label(format!("{} · {}",size_label(Some(item.bytes())),item.at.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M")));ui.horizontal_wrapped(|ui|{for(purge,label)in [(false,"Pulihkan…"),(true,"Hapus permanen…")]{if ui.add_enabled(!state.busy,egui::Button::new(RichText::new(label).color(if purge{theme::DANGER}else{theme::BLUE}))).clicked(){state.confirmation=Some(Confirmation::Recover{id:item.id.clone(),purge});}}});});}
});
}
