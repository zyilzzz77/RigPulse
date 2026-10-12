use crate::theme;
use eframe::egui::{self, RichText};
use rigpulse_core::{Page, Settings};

pub struct RigPulseApp {
    pub(crate) network_view: crate::pages::NetworkView,
    network_close_pending: bool,
    pub(crate) driver_admin: bool,
    pub(crate) admin_busy: bool,
    pub(crate) download_only: bool,
    pub(crate) restart_confirm: bool,
    pub(crate) wua_cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub(crate) install_cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub(crate) driver_progress: Option<rigpulse_drivers::install::Progress>,
    pub(crate) search_progress: Option<rigpulse_drivers::install::Progress>,
    pub(crate) network: crate::network_service::Controller,
    pub(crate) battery_history: std::collections::VecDeque<(chrono::DateTime<chrono::Utc>, f64)>,
    pub(crate) notifications: crate::notifications::Notifications,
    pub(crate) view_revision: u64,
    pub(crate) hardware_view: crate::pages::HardwareView,
    pub(crate) driver_view: crate::pages::DriverView,
    pub(crate) report_view: crate::pages::ReportView,
    pub(crate) chart_view: crate::pages::ChartView,
    pub(crate) page: Page,
    pub(crate) settings: Settings,
    pub(crate) dir: std::path::PathBuf,
    pub(crate) notice: Option<String>,
    pub(crate) inventory: Option<rigpulse_core::Inventory>,
    pub(crate) battery: Option<rigpulse_core::BatteryReading>,
    pub(crate) query: String,
    pub(crate) category: usize,
    pub(crate) busy: bool,
    pub(crate) monitor: rigpulse_monitor::Monitor,
    pub(crate) sample: Option<rigpulse_monitor::Sample>,
    pub(crate) history: rigpulse_monitor::buffer::RingBuffer,
    pub(crate) paused: bool,
    pub(crate) frozen_history: Option<rigpulse_monitor::buffer::RingBuffer>,
    pub(crate) combined_chart: bool,
    pub(crate) reset_confirm: bool,
    pub(crate) app_updater: crate::updater::Controller,
    pub(crate) apps: crate::apps_service::Controller,
    logo: Option<egui::TextureHandle>,
    pub(crate) minutes: i64,
    pub(crate) devices: Option<Vec<rigpulse_core::DeviceDriverInfo>>,
    pub(crate) updates: Option<rigpulse_core::UpdateSearch>,
    pub(crate) wua_busy: bool,
    pub(crate) wua_error: Option<String>,
    pub(crate) device_query: String,
    pub(crate) driver_status: usize,
    pub(crate) driver_class: String,
    pub(crate) selected_device: Option<usize>,
    pub(crate) diagnostics: Option<rigpulse_core::DiagnosticRun>,
    pub(crate) diagnostic_busy: bool,
    pub(crate) diagnostic_days: u32,
    pub(crate) scans: Vec<rigpulse_database::HistoryEntry>,
    pub(crate) report_busy: bool,
    pub(crate) delete_confirm: bool,
    pub(crate) db_busy: bool,
    pub(crate) last_export: Option<std::path::PathBuf>,
    pub(crate) install_selection: Vec<String>,
    pub(crate) install_confirm: bool,
    pub(crate) install_ack: bool,
    pub(crate) install_busy: bool,
    pub(crate) install_status: Option<rigpulse_drivers::install::InstallState>,
    pub(crate) install_result: Option<rigpulse_drivers::install::InstallationResult>,
    tx: std::sync::mpsc::SyncSender<crate::service::Event>,
    rx: std::sync::mpsc::Receiver<crate::service::Event>,
    #[cfg(feature = "qa-screenshot")]
    qa: crate::qa::Capture,
    #[cfg(feature = "qa-screenshot")]
    native_profile: crate::qa::NativeProfile,
    #[cfg(test)]
    nav_rects: [egui::Rect; 9],
}
impl RigPulseApp {
    pub fn new(cc: &eframe::CreationContext<'_>, dir: std::path::PathBuf) -> Self {
        Self::from_context(&cc.egui_ctx, dir)
    }
    fn from_context(ctx: &egui::Context, dir: std::path::PathBuf) -> Self {
        theme::install(ctx);
        let (settings, notice) = match Settings::load(&dir) {
            Ok(s) => (s, None),
            Err(_) => (
                Settings::default(),
                Some("Preferensi tidak dapat dibaca; menggunakan pengaturan awal.".into()),
            ),
        };
        let (tx, rx) = std::sync::mpsc::sync_channel(16);
        theme::apply_scale(ctx, settings.interface_scale);
        let minutes = settings.history_minutes;
        let logo =
            image::load_from_memory(include_bytes!("../../../RigPulse Neon Monitoring Logo.png"))
                .ok()
                .map(|image| {
                    let rgba = image
                        .resize(128, 128, image::imageops::FilterType::Lanczos3)
                        .to_rgba8();
                    ctx.load_texture(
                        "rigpulse_logo",
                        egui::ColorImage::from_rgba_unmultiplied(
                            [rgba.width() as usize, rgba.height() as usize],
                            rgba.as_raw(),
                        ),
                        egui::TextureOptions::LINEAR,
                    )
                });
        crate::service::scan(tx.clone(), ctx.clone());
        crate::service::load_history(dir.clone(), tx.clone(), ctx.clone());
        let repaint = ctx.clone();
        let monitor = rigpulse_monitor::Monitor::start(settings.power_saver, move || {
            repaint.request_repaint()
        });
        Self {
            network_view: Default::default(),
            network_close_pending: false,
            driver_admin: std::env::args().any(|s| s == "--driver-admin"),
            admin_busy: false,
            download_only: false,
            restart_confirm: false,
            wua_cancel: Default::default(),
            install_cancel: Default::default(),
            driver_progress: None,
            search_progress: None,
            network: crate::network_service::Controller::new(&settings, ctx.clone()),
            battery_history: Default::default(),
            notifications: Default::default(),
            view_revision: 0,
            hardware_view: Default::default(),
            driver_view: Default::default(),
            report_view: Default::default(),
            chart_view: Default::default(),
            page: if std::env::args().any(|s| s == "--driver-admin") {
                Page::Drivers
            } else {
                initial_page()
            },
            apps: crate::apps_service::Controller::new(dir.clone()),
            settings,
            dir,
            notice,
            inventory: None,
            battery: None,
            query: String::new(),
            category: 0,
            busy: true,
            tx,
            rx,
            monitor,
            sample: None,
            history: Default::default(),
            paused: false,
            frozen_history: None,
            combined_chart: false,
            reset_confirm: false,
            app_updater: crate::updater::Controller::new(),
            logo,
            minutes,
            devices: None,
            updates: None,
            wua_busy: false,
            wua_error: None,
            device_query: String::new(),
            driver_status: 0,
            driver_class: String::new(),
            selected_device: None,
            diagnostics: None,
            diagnostic_busy: false,
            diagnostic_days: 7,
            scans: Vec::new(),
            report_busy: false,
            delete_confirm: false,
            db_busy: false,
            last_export: None,
            install_selection: Vec::new(),
            install_confirm: false,
            install_ack: false,
            install_busy: false,
            install_status: None,
            install_result: None,
            #[cfg(feature = "qa-screenshot")]
            qa: crate::qa::Capture::new(),
            #[cfg(feature = "qa-screenshot")]
            native_profile: crate::qa::NativeProfile::new(),
            #[cfg(test)]
            nav_rects: [egui::Rect::NOTHING; 9],
        }
    }
    pub(crate) fn scan(&mut self, ctx: &egui::Context) {
        if !self.busy {
            self.busy = true;
            crate::service::scan(self.tx.clone(), ctx.clone());
        }
    }
    pub(crate) fn check_updates(&mut self, ctx: &egui::Context) {
        if !self.wua_busy && !self.install_busy && !self.admin_busy {
            self.wua_busy = true;
            self.wua_error = None;
            self.search_progress = None;
            self.wua_cancel = Default::default();
            crate::service::search(self.wua_cancel.clone(), self.tx.clone(), ctx.clone());
        }
    }
    pub(crate) fn diagnose(&mut self, ctx: &egui::Context) {
        if !self.diagnostic_busy {
            self.diagnostic_busy = true;
            crate::service::diagnose(
                self.diagnostic_days,
                self.dir.clone(),
                self.tx.clone(),
                ctx.clone(),
            );
        }
    }
    pub(crate) fn start_install(
        &mut self,
        consent: rigpulse_drivers::install::Consent,
        ctx: &egui::Context,
    ) {
        self.install_busy = true;
        self.install_status = Some(rigpulse_drivers::install::InstallState::AwaitingConfirmation);
        self.install_cancel = Default::default();
        self.driver_progress = None;
        self.install_result = None;
        crate::service::install(
            consent,
            self.download_only,
            self.install_cancel.clone(),
            self.dir.clone(),
            self.tx.clone(),
            ctx.clone(),
        );
    }
    pub(crate) fn open_driver_admin(&mut self, ctx: &egui::Context) {
        if !self.admin_busy {
            self.admin_busy = true;
            crate::service::open_admin(self.tx.clone(), ctx.clone());
        }
    }
    pub(crate) fn export_report(&mut self, pdf: bool, ctx: &egui::Context) {
        if self.report_busy {
            return;
        }
        self.report_busy = true;
        let report = rigpulse_reports::Report {
            app_version: env!("CARGO_PKG_VERSION").into(),
            exported_at: chrono::Utc::now(),
            sensitive_included: false,
            inventory: self.inventory.clone(),
            battery: self.battery.clone(),
            devices: self.devices.clone(),
            updates: self.updates.clone(),
            monitoring: self.sample.clone(),
            diagnostics: self.diagnostics.clone(),
        }
        .private(self.settings.include_sensitive);
        crate::service::export(report, pdf, self.dir.clone(), self.tx.clone(), ctx.clone());
    }
    pub(crate) fn delete_history(&mut self, ctx: &egui::Context) {
        self.db_busy = true;
        crate::service::delete_history(self.dir.clone(), self.tx.clone(), ctx.clone());
    }
    pub(crate) fn load_scan(&mut self, id: i64, ctx: &egui::Context) {
        crate::service::load_scan(id, self.dir.clone(), self.tx.clone(), ctx.clone());
    }
    fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("sidebar")
            .exact_width(224.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(theme::SIDEBAR).inner_margin(16))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(logo) = &self.logo {
                        ui.image((logo.id(), egui::vec2(40.0, 40.0)));
                    }
                    ui.label(RichText::new("RigPulse").size(25.0).strong());
                });
                ui.label(
                    RichText::new("Kenali Perangkat Anda")
                        .size(12.0)
                        .color(theme::MUTED),
                );
                ui.add_space(20.0);
                egui::ScrollArea::vertical()
                    .id_salt("sidebar_scroll")
                    .show(ui, |ui| {
                        ui.visuals_mut().widgets.inactive.weak_bg_fill = theme::SIDEBAR;
                        ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::NONE;
                        for page in Page::ALL {
                            if self.driver_admin && page != Page::Drivers {
                                continue;
                            }
                            let active = self.page == page;
                            let label = RichText::new(format!("       {}", page.title()))
                                .color(if active { theme::BLUE } else { theme::TEXT });
                            let button = egui::Button::new(label).right_text("").selected(active);
                            let button = if active {
                                button.fill(theme::HOVER)
                            } else {
                                button
                            };
                            let response = ui
                                .add_sized([ui.available_width(), 40.0], button)
                                .on_hover_text(page.description());
                            let icon_rect = egui::Rect::from_center_size(
                                egui::pos2(response.rect.left() + 22.0, response.rect.center().y),
                                egui::vec2(20.0, 20.0),
                            );
                            theme::paint_icon(
                                ui.painter(),
                                icon_rect,
                                page,
                                if active { theme::BLUE } else { theme::MUTED },
                            );
                            if response.clicked() {
                                self.page = page;
                            }
                            #[cfg(test)]
                            {
                                self.nav_rects[page as usize] = response.rect;
                            }
                        }
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(format!(
                                "v{} · Lokal & privat",
                                env!("CARGO_PKG_VERSION")
                            ))
                            .size(12.0)
                            .color(theme::MUTED),
                        );
                    });
            });
    }
}
impl RigPulseApp {
    fn render(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && self.network.busy {
            self.network.cancel();
            self.network_close_pending = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.notifications.push(
                crate::notifications::Kind::Information,
                "Membatalkan pengujian jaringan sebelum menutup RigPulse…",
            );
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.install_busy {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.notifications.push(crate::notifications::Kind::Warning,"Operasi driver masih berjalan. Minta pembatalan dan tunggu hasil Windows sebelum menutup RigPulse.");
        }
        #[cfg(feature = "qa-screenshot")]
        if std::env::var_os("RIGPULSE_QA_PAGE").is_some() {
            self.page = initial_page();
        }
        #[cfg(feature = "qa-screenshot")]
        self.qa.update(ctx);
        if !self.driver_admin {
            self.app_updater
                .automatic(&mut self.settings, &self.dir, ctx);
            self.apps.poll(ctx, &self.settings);
        }
        for (kind, text) in self.app_updater.notifications.drain(..) {
            self.notifications.push(kind, text);
        }
        self.network.poll(&self.settings);
        if self.network_close_pending && !self.network.busy && !self.install_busy {
            self.network_close_pending = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        for (kind, text) in self.network.notifications.drain(..) {
            self.notifications.push(kind, text);
        }
        for (kind, text) in self.apps.notifications.drain(..) {
            self.notifications.push(kind, text);
        }
        #[cfg(feature = "qa-screenshot")]
        {
            if std::env::var("RIGPULSE_QA_NETWORK_TAB").ok().as_deref() == Some("1") {
                self.network.tab = 1;
            }
            if let Some(tab) = std::env::var("RIGPULSE_QA_APPS_TAB")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
            {
                self.apps.tab = tab.min(2);
            }
            if let Ok(name) = std::env::var("RIGPULSE_QA_APP_DETAIL")
                && !name.trim().is_empty()
                && let Some(inv) = &self.apps.inventory
                && let Some(app) = inv
                    .apps
                    .iter()
                    .find(|a| a.name.to_lowercase().contains(&name.to_lowercase()))
            {
                self.apps.selected = Some(app.id.clone());
                self.apps.details_open = true;
            }
        }
        let animation_time = if self.settings.reduced_animations {
            0.0
        } else {
            0.1
        };
        if ctx.style().animation_time != animation_time {
            ctx.style_mut(|style| style.animation_time = animation_time);
        }
        while let Ok(event) = self.rx.try_recv() {
            match event {
                crate::service::Event::Inventory(i) => {
                    self.inventory = Some(i);
                    self.view_revision = self.view_revision.wrapping_add(1);
                }
                crate::service::Event::Battery(b) => self.battery = Some(b),
                crate::service::Event::Error(e) => self
                    .notifications
                    .push(crate::notifications::Kind::Error, e),
                crate::service::Event::ScanDone => {
                    self.busy = false;
                    self.notice=Some("Pemindaian perangkat selesai. Data yang tidak tersedia ditandai pada masing-masing bagian.".into());
                }
                crate::service::Event::Devices(d) => {
                    self.view_revision = self.view_revision.wrapping_add(1);
                    self.devices = Some(d);
                    self.selected_device = None;
                }
                crate::service::Event::Updates(result) => {
                    self.wua_busy = false;
                    match result {
                        Ok(s) => {
                            self.install_selection.clear();
                            if self.settings.driver_notifications {
                                self.notifications.push(
                                    crate::notifications::Kind::Success,
                                    format!(
                                        "Pemeriksaan selesai: {} penawaran driver",
                                        s.updates.len()
                                    ),
                                );
                            }
                            self.updates = Some(s);
                        }
                        Err(e) => {
                            if self.settings.driver_notifications {
                                self.notifications
                                    .push(crate::notifications::Kind::Error, e.clone());
                            }
                            self.wua_error = Some(e);
                        }
                    }
                }
                crate::service::Event::Diagnostics(run) => {
                    self.notifications.push(
                        crate::notifications::Kind::Success,
                        "Pemeriksaan diagnostik selesai",
                    );
                    self.diagnostic_busy = false;
                    self.diagnostics = Some(run);
                }
                crate::service::Event::History(entries) => {
                    self.scans = entries;
                    self.view_revision = self.view_revision.wrapping_add(1);
                }
                crate::service::Event::Export(result) => {
                    self.report_busy = false;
                    match result {
                        Ok(p) => {
                            self.notice = Some(format!("Laporan dibuat: {}", p.display()));
                            self.last_export = Some(p);
                        }
                        Err(e) => self.notice = Some(e),
                    }
                }
                crate::service::Event::HistoryDeleted(result) => {
                    self.db_busy = false;
                    match result {
                        Ok(()) => {
                            self.scans.clear();
                            self.view_revision = self.view_revision.wrapping_add(1);
                            self.diagnostics = None;
                            self.notice=Some("Riwayat dihapus. File laporan yang sudah diekspor tetap berada di folder laporan.".into());
                        }
                        Err(e) => self.notice = Some(format!("Riwayat gagal dihapus: {e}")),
                    }
                }
                crate::service::Event::HistoryLoaded(result) => match result {
                    Ok(r) => {
                        self.diagnostics = Some(r);
                        self.page = Page::Diagnostics;
                    }
                    Err(e) => self.notice = Some(format!("Gagal membuka riwayat: {e}")),
                },
                crate::service::Event::InstallProgress(progress) => {
                    self.install_status = Some(progress.state);
                    self.driver_progress = Some(progress);
                }
                crate::service::Event::SearchProgress(progress) => {
                    self.search_progress = Some(progress)
                }
                crate::service::Event::AdminClosed(result) => {
                    self.admin_busy = false;
                    match result {
                        Ok(()) => {
                            self.scan(ctx);
                            self.check_updates(ctx);
                        }
                        Err(e) => self.notifications.push(
                            crate::notifications::Kind::Error,
                            format!("Jendela administrator tidak dapat dibuka: {e}"),
                        ),
                    }
                }
                crate::service::Event::InstallDone(result) => {
                    self.install_busy = false;
                    match result {
                        Ok(r) => {
                            if self.settings.driver_notifications {
                                self.notifications.push(
                                    if matches!(
                                        r.state,
                                        rigpulse_drivers::install::InstallState::Completed
                                            | rigpulse_drivers::install::InstallState::Downloaded
                                    ) {
                                        crate::notifications::Kind::Success
                                    } else {
                                        crate::notifications::Kind::Warning
                                    },
                                    format!(
                                        "Operasi driver: {}",
                                        crate::pages::drivers::state_label(r.state)
                                    ),
                                );
                            }
                            self.install_status = Some(r.state);
                            self.install_result = Some(r);
                            self.scan(ctx);
                            self.check_updates(ctx);
                        }
                        Err(e) => {
                            self.notifications.push(
                                crate::notifications::Kind::Error,
                                format!("Operasi driver gagal: {e}"),
                            );
                            self.install_status =
                                Some(rigpulse_drivers::install::InstallState::Failed);
                            self.notice = Some(format!(
                                "Instalasi tidak selesai: {e}. Jika akses admin ditolak, gunakan Windows Update."
                            ));
                        }
                    }
                }
            }
        }
        self.monitor
            .set_interval_seconds(self.settings.interval_seconds());
        while let Ok(sample) = self.monitor.rx.try_recv() {
            if let (Some(cpu), Some(ram)) = (sample.cpu.value, sample.memory.value) {
                self.history.push(sample.cpu.at, cpu, ram);
            }
            if let Some(b) = &sample.battery {
                if let Some(watts) = b.watts()
                    && self
                        .battery_history
                        .back()
                        .is_none_or(|(at, _)| *at != b.at)
                {
                    self.battery_history.push_back((b.at, watts));
                    while self.battery_history.len() > 720 {
                        self.battery_history.pop_front();
                    }
                }
                self.battery = Some(b.clone());
            }
            self.sample = Some(sample);
        }
        self.sidebar(ctx);
        if let Some(text) = self.notice.take() {
            self.notifications
                .push(crate::notifications::Kind::Information, text);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BACKGROUND).inner_margin(
                if ctx.screen_rect().width() < 900.0 {
                    16
                } else {
                    24
                },
            ))
            .show(ctx, |ui| {
                ui.heading(self.page.title());
                ui.label(RichText::new(self.page.description()).color(theme::MUTED));
                ui.add_space(20.0);
                if let Some(n) = self.notice.clone() {
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(theme::BLUE, n);
                        if ui.small_button("Tutup").clicked() {
                            self.notice = None;
                        }
                    });
                }
                let scroll = egui::ScrollArea::vertical()
                    .id_salt(self.page as usize)
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(
                        egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                    );
                #[cfg(feature = "qa-screenshot")]
                let scroll = if let Some(offset) = std::env::var("RIGPULSE_QA_SCROLL")
                    .ok()
                    .and_then(|s| s.parse::<f32>().ok())
                {
                    scroll.vertical_scroll_offset(offset)
                } else {
                    scroll
                };
                scroll.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    self.content(ui, ctx);
                });
            });
        self.notifications.show(ctx);
    }
}
impl eframe::App for RigPulseApp {
    #[cfg(feature = "qa-screenshot")]
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        let ready = !self.busy && (self.page != Page::Apps || self.apps.inventory.is_some());
        self.native_profile.input(ctx, input, ready);
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(feature = "qa-screenshot")]
        let started = std::time::Instant::now();
        self.render(ctx);
        #[cfg(feature = "qa-screenshot")]
        self.native_profile.record(
            ctx,
            started.elapsed(),
            [
                self.apps.inventory.as_ref().map_or(0, |i| i.apps.len()),
                self.devices.as_ref().map_or(0, Vec::len),
                self.inventory
                    .as_ref()
                    .map_or(0, |i| i.sections.iter().map(|s| s.properties.len()).sum()),
            ],
        );
    }
}
fn initial_page() -> Page {
    #[cfg(feature = "qa-screenshot")]
    if let Ok(name) = std::env::var("RIGPULSE_QA_PAGE")
        && let Some(p) = Page::ALL
            .into_iter()
            .find(|p| p.title().eq_ignore_ascii_case(&name))
    {
        return p;
    }
    Page::Dashboard
}
#[cfg(test)]
#[path = "perf_tests.rs"]
mod perf_tests;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closing_cancels_network_job_and_waits_for_completion() {
        let ctx = egui::Context::default();
        let dir =
            std::env::temp_dir().join(format!("rigpulse-network-close-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = RigPulseApp::from_context(&ctx, dir);
        app.rx = std::sync::mpsc::sync_channel(16).1;
        app.network.busy = true;
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .events
            .push(egui::ViewportEvent::Close);
        let output = ctx.run(input, |ctx| app.render(ctx));
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&egui::ViewportCommand::CancelClose)
        );
        app.network.busy = false;
        let output = ctx.run(egui::RawInput::default(), |ctx| app.render(ctx));
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&egui::ViewportCommand::Close)
        );
    }
    #[test]
    fn apps_tabs_and_large_inventory_remain_interactive_at_narrow_and_wide_sizes() {
        for (width, height) in [(620.0, 1400.0), (1200.0, 900.0), (1920.0, 1080.0)] {
            let ctx = egui::Context::default();
            let dir = std::env::temp_dir()
                .join(format!("rigpulse-apps-ui-{}-{width}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let mut app = RigPulseApp::from_context(&ctx, dir);
            app.rx = std::sync::mpsc::sync_channel(16).1;
            app.busy = false;
            app.notice = None;
            app.page = Page::Apps;
            app.apps.inventory = Some(rigpulse_apps::Inventory {
                apps: (0..1000)
                    .map(|n| rigpulse_apps::Application {
                        id: format!("test-{n}"),
                        name: format!("Application with a long display name {n}"),
                        publisher: Some("Fixture publisher".into()),
                        ..Default::default()
                    })
                    .collect(),
                scanned_at: chrono::Utc::now(),
                warnings: Vec::new(),
                completed_sources: vec!["fixture".into()],
            });
            {
                app.apps.selected = Some("test-0".into());
                app.apps.details_open = true;
            }
            let input = || egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, height),
                )),
                ..Default::default()
            };
            let _ = ctx.run(input(), |ctx| app.render(ctx));
            {
                assert!(app.apps.details_panel_rect.height() >= 220.0);
                assert!(app.apps.details_panel_rect.width() > 300.0);
                assert!(
                    app.apps.details_panel_rect.width() <= width * 0.9 + 1.0,
                    "modal {} vs viewport {}",
                    app.apps.details_panel_rect.width(),
                    width
                );
                let mut escape = input();
                escape.events.push(egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                });
                let _ = ctx.run(escape, |ctx| app.render(ctx));
                assert!(!app.apps.details_open);
                let _ = ctx.run(input(), |ctx| app.render(ctx));
            }
            for tab in [1, 2, 0] {
                let point = app.apps.tab_rects[tab].center();
                let mut down = input();
                down.events = vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                ];
                let _ = ctx.run(down, |ctx| app.render(ctx));
                let mut up = input();
                up.events = vec![egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }];
                let output = ctx.run(up, |ctx| app.render(ctx));
                assert_eq!(app.apps.tab, tab);
                assert!(!output.shapes.is_empty());
                assert!(app.apps.confirmation.is_none());
            }
        }
    }
    #[test]
    fn sidebar_clicks_navigate_all_pages_at_supported_sizes() {
        for (width, scale) in [
            (620.0, 1.0),
            (780.0, 1.25),
            (1024.0, 1.5),
            (1280.0, 1.0),
            (1366.0, 1.25),
            (1600.0, 1.5),
            (1920.0, 1.75),
            (2560.0, 1.0),
        ] {
            let ctx = egui::Context::default();
            let dir =
                std::env::temp_dir().join(format!("rigpulse-ui-{}-{width}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let mut app = RigPulseApp::from_context(&ctx, dir);
            app.rx = std::sync::mpsc::sync_channel(16).1;
            app.busy = false;
            app.notice = None;
            let input = || {
                let mut input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 780.0),
                    )),
                    ..Default::default()
                };
                input
                    .viewports
                    .get_mut(&egui::ViewportId::ROOT)
                    .unwrap()
                    .native_pixels_per_point = Some(scale);
                input
            };
            let _ = ctx.run(input(), |ctx| app.render(ctx));
            assert!((ctx.pixels_per_point() - scale).abs() < 0.01);
            for page in Page::ALL {
                let point = app.nav_rects[page as usize].center();
                let mut down = input();
                down.events = vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                ];
                let _ = ctx.run(down, |ctx| app.render(ctx));
                let mut up = input();
                up.events = vec![egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }];
                let output = ctx.run(up, |ctx| app.render(ctx));
                assert_eq!(app.page, page);
                assert!(!output.shapes.is_empty());
            }
        }
    }
    #[test]
    fn common_physical_resolutions_and_dpi_render_all_pages() {
        let ctx = egui::Context::default();
        let dir = std::env::temp_dir().join(format!("rigpulse-resolution-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = RigPulseApp::from_context(&ctx, dir);
        app.rx = std::sync::mpsc::sync_channel(16).1;
        app.busy = false;
        app.notice = None;
        for (width, height) in [
            (1280.0, 720.0),
            (1366.0, 768.0),
            (1600.0, 900.0),
            (1920.0, 1080.0),
            (2560.0, 1440.0),
        ] {
            for scale in [1.0, 1.25, 1.5, 1.75] {
                for page in Page::ALL {
                    app.page = page;
                    let mut input = egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width / scale, height / scale),
                        )),
                        ..Default::default()
                    };
                    input
                        .viewports
                        .get_mut(&egui::ViewportId::ROOT)
                        .unwrap()
                        .native_pixels_per_point = Some(scale);
                    let output = ctx.run(input, |ctx| app.render(ctx));
                    assert!(!output.shapes.is_empty());
                    assert!((ctx.pixels_per_point() - scale).abs() < 0.01);
                    if page == Page::Monitoring {
                        app.network.tab = 1;
                        let output = ctx.run(
                            egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    egui::vec2(width / scale, height / scale),
                                )),
                                ..Default::default()
                            },
                            |ctx| app.render(ctx),
                        );
                        assert!(!output.shapes.is_empty());
                        app.network.tab = 0;
                    }
                }
            }
        }
    }
}
