use eframe::egui;
use rigpulse_apps::{
    Application, Inventory,
    cleanup::{QuarantineEntry, ScanResult},
    query::{Filter, Sort},
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::Instant,
};

#[derive(Debug, Clone)]
#[allow(
    clippy::large_enum_variant,
    reason = "One immutable confirmation; no collection of large enum values"
)]
pub enum Confirmation {
    Uninstall {
        app: Application,
        advanced: bool,
        elevate: bool,
    },
    Clean {
        ids: Vec<String>,
    },
    Recover {
        id: String,
        purge: bool,
    },
}
#[allow(
    clippy::large_enum_variant,
    reason = "Single-slot queue bounds owned application metadata"
)]
enum Job {
    Inventory,
    OpenFolder(PathBuf),
    Uninstall {
        app: Application,
        advanced: bool,
        elevate: bool,
        auto_scan: bool,
        registry: bool,
    },
    Scan {
        registry: bool,
    },
    Clean {
        candidates: Vec<rigpulse_apps::cleanup::Candidate>,
        ids: Vec<String>,
        dry_run: bool,
    },
    Recover {
        id: String,
        purge: bool,
    },
}
struct Request {
    job: Job,
    cancel: Arc<AtomicBool>,
    ctx: egui::Context,
}
enum Event {
    Notice(crate::notifications::Kind, String),
    ReviewReady,
    Progress(String),
    Inventory(Inventory, Vec<(String, rigpulse_apps::icons::Icon)>),
    Scan(ScanResult),
    Quarantine(Vec<QuarantineEntry>),
    CleanupFailures(Vec<String>),
    Pending(usize),
    Done(String),
    Failed(String, bool),
}

pub struct Controller {
    pub notifications: Vec<(crate::notifications::Kind, String)>,
    pub cleanup_review_open: bool,
    #[cfg(test)]
    pub tab_rects: [egui::Rect; 3],
    #[cfg(test)]
    pub details_panel_rect: egui::Rect,
    tx: SyncSender<Request>,
    rx: Receiver<Event>,
    cancel: Arc<AtomicBool>,
    pub inventory: Option<Inventory>,
    pub scan: Option<ScanResult>,
    pub quarantine: Vec<QuarantineEntry>,
    pub cleanup_failures: Vec<String>,
    pub busy: bool,
    pub status: Option<String>,
    pub error: Option<String>,
    pub technical_error: Option<String>,
    pub permission_denied: bool,
    pub tab: usize,
    pub query: String,
    pub filter: Filter,
    pub sort: Sort,
    pub selected: Option<String>,
    pub selected_candidates: Vec<String>,
    pub confirmation: Option<Confirmation>,
    pub last_uninstall: Option<(Application, bool)>,
    pub pending: usize,
    pub icons: HashMap<String, egui::TextureHandle>,
    pub filtered: Vec<usize>,
    pub details_open: bool,
    pub quarantine_page: usize,
    last_query: String,
    applied_query: String,
    query_changed: Instant,
    last_filter: Filter,
    last_sort: Sort,
    dirty: bool,
    last_background: Instant,
    overview_key: Option<(chrono::DateTime<chrono::Utc>, usize)>,
    overview_labels: [String; 3],
}
impl Controller {
    pub fn new(dir: PathBuf) -> Self {
        let (tx, commands) = mpsc::sync_channel(1);
        let (events, rx) = mpsc::sync_channel(8);
        std::thread::spawn(move || worker(dir, commands, events));
        Self {
            notifications: Vec::new(),
            cleanup_review_open: false,
            #[cfg(test)]
            tab_rects: [egui::Rect::NOTHING; 3],
            #[cfg(test)]
            details_panel_rect: egui::Rect::NOTHING,
            tx,
            rx,
            cancel: Arc::new(AtomicBool::new(false)),
            inventory: None,
            scan: None,
            quarantine: Vec::new(),
            cleanup_failures: Vec::new(),
            busy: false,
            status: None,
            error: None,
            technical_error: None,
            permission_denied: false,
            tab: 0,
            query: String::new(),
            filter: Filter::All,
            sort: Sort::Name,
            selected: None,
            selected_candidates: Vec::new(),
            confirmation: None,
            last_uninstall: None,
            pending: 0,
            icons: HashMap::new(),
            filtered: Vec::new(),
            details_open: false,
            quarantine_page: 0,
            last_query: String::new(),
            applied_query: String::new(),
            query_changed: Instant::now(),
            last_filter: Filter::All,
            last_sort: Sort::Name,
            dirty: true,
            last_background: Instant::now(),
            overview_key: None,
            overview_labels: [
                "Belum dipindai".into(),
                "Tidak tersedia".into(),
                "Belum pernah".into(),
            ],
        }
    }
    fn submit(&mut self, job: Job, ctx: &egui::Context) {
        if self.busy {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        match self.tx.try_send(Request {
            job,
            cancel: cancel.clone(),
            ctx: ctx.clone(),
        }) {
            Ok(()) => {
                self.cancel = cancel;
                self.busy = true;
                self.status = Some("Menyiapkan tugas…".into());
                self.error = None;
                self.technical_error = None;
                self.cleanup_failures.clear();
                self.permission_denied = false;
            }
            Err(_) => {
                self.error = Some(
                    "Tugas belum dapat dimulai. Coba lagi setelah tugas sebelumnya selesai.".into(),
                )
            }
        }
    }
    pub fn refresh(&mut self, ctx: &egui::Context) {
        self.submit(Job::Inventory, ctx);
    }
    pub fn open_folder(&mut self, path: PathBuf, ctx: &egui::Context) {
        self.submit(Job::OpenFolder(path), ctx);
    }
    pub fn overview(&mut self) -> [String; 3] {
        let key = self
            .inventory
            .as_ref()
            .map(|i| (i.scanned_at, i.apps.len()));
        if self.overview_key != key {
            self.overview_labels = if let Some(inventory) = &self.inventory {
                let mut count = 0;
                let mut known = 0;
                let mut bytes = 0u64;
                for app in inventory.apps.iter().filter(|a| !a.protected) {
                    count += 1;
                    if let Some(size) = app.estimated_bytes {
                        known += 1;
                        bytes = bytes.saturating_add(size);
                    }
                }
                [
                    count.to_string(),
                    if known == 0 {
                        "Tidak tersedia".into()
                    } else {
                        format!("{} · {} app", rigpulse_apps::size_label(Some(bytes)), known)
                    },
                    inventory
                        .scanned_at
                        .with_timezone(&chrono::Local)
                        .format("%d/%m %H:%M")
                        .to_string(),
                ]
            } else {
                [
                    "Belum dipindai".into(),
                    "Tidak tersedia".into(),
                    "Belum pernah".into(),
                ]
            };
            self.overview_key = key;
        }
        self.overview_labels.clone()
    }
    pub fn scan_leftovers(&mut self, ctx: &egui::Context, registry: bool) {
        self.submit(Job::Scan { registry }, ctx);
    }
    pub fn cancel(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.status = Some(
            "Membatalkan pada batas operasi yang aman. Uninstaller eksternal dapat tetap terbuka."
                .into(),
        );
    }
    pub fn dry_run(&mut self, ctx: &egui::Context) {
        if let Some(scan) = &self.scan {
            self.submit(
                Job::Clean {
                    candidates: scan.candidates.clone(),
                    ids: self.selected_candidates.clone(),
                    dry_run: true,
                },
                ctx,
            );
        }
    }
    pub fn approve(
        &mut self,
        confirmation: Confirmation,
        ctx: &egui::Context,
        settings: &rigpulse_core::Settings,
    ) {
        match confirmation {
            Confirmation::Uninstall {
                app,
                advanced,
                elevate,
            } => {
                self.last_uninstall = Some((app.clone(), advanced));
                self.submit(
                    Job::Uninstall {
                        app,
                        advanced,
                        elevate,
                        auto_scan: settings.apps_auto_scan,
                        registry: settings.apps_registry_review,
                    },
                    ctx,
                );
            }
            Confirmation::Clean { ids } => {
                if let Some(scan) = &self.scan {
                    self.submit(
                        Job::Clean {
                            candidates: scan.candidates.clone(),
                            ids,
                            dry_run: false,
                        },
                        ctx,
                    );
                }
            }
            Confirmation::Recover { id, purge } => self.submit(Job::Recover { id, purge }, ctx),
        }
    }
    pub fn poll(&mut self, ctx: &egui::Context, settings: &rigpulse_core::Settings) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Notice(kind, text) => self.notifications.push((kind, text)),
                Event::ReviewReady => self.cleanup_review_open = true,
                Event::Progress(text) => self.status = Some(text),
                Event::Inventory(inv, icons) => {
                    if self
                        .selected
                        .as_ref()
                        .is_some_and(|id| !inv.apps.iter().any(|a| &a.id == id))
                    {
                        self.selected = None;
                        self.details_open = false;
                    }
                    self.inventory = Some(inv);
                    self.dirty = true;
                    if !icons.is_empty() {
                        self.icons.clear();
                    }
                    for (id, icon) in icons {
                        self.icons.insert(
                            id.clone(),
                            ctx.load_texture(
                                id,
                                egui::ColorImage::from_rgba_unmultiplied(
                                    [icon.width, icon.height],
                                    &icon.rgba,
                                ),
                                egui::TextureOptions::LINEAR,
                            ),
                        );
                    }
                }
                Event::Scan(mut scan) => {
                    if settings.apps_preserve_data {
                        scan.candidates
                            .retain(|c| c.category != "Konfigurasi / data pengguna");
                    }
                    self.scan = Some(scan);
                    self.selected_candidates.clear();
                    self.tab = 2;
                }
                Event::Quarantine(entries) => self.quarantine = entries,
                Event::CleanupFailures(items) => self.cleanup_failures = items,
                Event::Pending(count) => self.pending = count,
                Event::Done(message) => {
                    self.busy = false;
                    self.status = Some(message);
                }
                Event::Failed(detail, permission) => {
                    self.notifications.push((
                        crate::notifications::Kind::Error,
                        "Tugas Apps gagal. Buka rincian kegagalan.".into(),
                    ));
                    self.busy = false;
                    self.status = None;
                    self.error=Some(if self.cancel.load(Ordering::Relaxed){"Tugas dibatalkan. Tidak ada pembersihan lanjutan otomatis."}else if permission{"Akses ditolak. Uninstaller tertentu memerlukan persetujuan administrator."}else{"Tugas tidak selesai. Periksa rincian dan coba lagi."}.into());
                    self.technical_error = Some(detail);
                    self.permission_denied = permission;
                }
            }
        }
        if self.last_query != self.query {
            self.last_query = self.query.clone();
            self.query_changed = Instant::now();
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        let filters_changed = self.filter != self.last_filter || self.sort != self.last_sort;
        if self.dirty || filters_changed || self.filtered_search_changed() {
            if let Some(inventory) = &self.inventory {
                let indexes: HashMap<&str, usize> = inventory
                    .apps
                    .iter()
                    .enumerate()
                    .map(|(n, a)| (a.id.as_str(), n))
                    .collect();
                self.filtered = rigpulse_apps::query::select(
                    &inventory.apps,
                    &self.query,
                    self.filter,
                    self.sort,
                )
                .iter()
                .filter_map(|a| indexes.get(a.id.as_str()).copied())
                .collect();
            }
            self.dirty = false;
            self.last_filter = self.filter;
            self.last_sort = self.sort;
            self.applied_query = self.query.clone();
        }
        if settings.apps_background_scan
            && !self.busy
            && self.last_background.elapsed().as_secs() >= 1800
        {
            self.last_background = Instant::now();
            self.scan_leftovers(ctx, settings.apps_registry_review);
        }
    }
    fn filtered_search_changed(&self) -> bool {
        self.query_changed.elapsed().as_millis() >= 250 && self.applied_query != self.query
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

fn worker(dir: PathBuf, commands: Receiver<Request>, events: SyncSender<Event>) {
    use rigpulse_apps::{
        cleanup::{self, Quarantine, ScanRoots},
        history::History,
        uninstall::{self, Outcome},
    };
    while let Ok(request) = commands.recv() {
        let Request { job, cancel, ctx } = request;
        let send = |event| {
            let _ = events.send(event);
            ctx.request_repaint();
        };
        let progress = |text: &str| {
            let _ = events.try_send(Event::Progress(text.into()));
            ctx.request_repaint();
        };
        let inventory = || {
            rigpulse_apps::inventory::scan(&cancel, |n| {
                if n % 64 == 0 {
                    progress(&format!("Memuat aplikasi terinstal… {n} entri"));
                }
            })
        };
        let result = (|| -> anyhow::Result<String> {
            let roots = ScanRoots::current(dir.clone())?;
            let mut history = History::load(&dir)?;
            match job {
                Job::OpenFolder(path) => {
                    rigpulse_apps::open_install_folder(&path)?;
                    Ok("Folder instalasi dibuka.".into())
                }
                Job::Inventory => {
                    let inv = inventory()?;
                    let mut changed = false;
                    for app in history.pending_restart.clone() {
                        if uninstall::removal_verified(&app, &inv) {
                            history.record_removed(app.clone());
                            history.pending_restart.retain(|a| a.id != app.id);
                            changed = true;
                        }
                    }
                    if changed {
                        history.save(&dir)?;
                    }
                    let mut icons = Vec::new();
                    for app in inv.apps.iter().filter(|a| !a.protected).take(128) {
                        if cancel.load(Ordering::Relaxed) {
                            anyhow::bail!("Dibatalkan");
                        }
                        if let Some(path) = &app.icon_path
                            && let Some(icon) = rigpulse_apps::icons::load(path)
                        {
                            icons.push((app.id.clone(), icon));
                        }
                    }
                    let count = inv.apps.iter().filter(|a| !a.protected).count();
                    let warning = inv.warnings.len();
                    send(Event::Inventory(inv, icons));
                    send(Event::Notice(
                        crate::notifications::Kind::Information,
                        "Daftar aplikasi telah diperbarui.".into(),
                    ));
                    send(Event::Quarantine(Quarantine::entries(&roots)?));
                    send(Event::Pending(history.pending_restart.len()));
                    Ok(format!(
                        "{count} aplikasi terdeteksi. {}",
                        if warning > 0 {
                            format!("{warning} sumber/entri tidak terbaca; lihat rincian sumber.")
                        } else {
                            "Pemindaian selesai.".into()
                        }
                    ))
                }
                Job::Uninstall {
                    app,
                    advanced,
                    elevate,
                    auto_scan,
                    registry,
                } => {
                    if !history.pending_restart.iter().any(|a| a.id == app.id) {
                        anyhow::ensure!(
                            history.pending_restart.len() < 32,
                            "Terlalu banyak uninstall tertunda"
                        );
                        history.pending_restart.push(app.clone());
                        history.save(&dir)?;
                    }
                    let result =
                        uninstall::execute_with_elevation(&app, true, elevate, &cancel, progress)?;
                    tracing::info!(app=%app.name,outcome=?result.outcome,code=?result.exit_code,"Apps official uninstall result");
                    let verified = result
                        .inventory
                        .as_ref()
                        .is_some_and(|i| uninstall::removal_verified(&app, i));
                    if verified {
                        send(Event::Notice(
                            crate::notifications::Kind::Success,
                            format!("{} berhasil di-uninstall.", app.name),
                        ));
                        if let Some(inv) = &result.inventory {
                            send(Event::Inventory(inv.clone(), Vec::new()));
                        }
                        history.record_removed(app.clone());
                        history.pending_restart.retain(|a| a.id != app.id);
                        history.save(&dir)?;
                        if advanced && auto_scan && result.outcome != Outcome::Cancelled {
                            progress("Memindai file sisa…");
                            let scan = cleanup::scan(
                                &history.removed,
                                result.inventory.as_ref().unwrap(),
                                &roots,
                                registry,
                                &cancel,
                                |n| progress(&format!("Memindai file sisa… {n} item")),
                            )?;
                            send(Event::Scan(scan));
                            send(Event::ReviewReady);
                        }
                    } else if result.outcome == Outcome::Cancelled {
                        history.pending_restart.retain(|a| a.id != app.id);
                        history.save(&dir)?;
                    }
                    if !verified && let Some(inv) = result.inventory {
                        send(Event::Inventory(inv, Vec::new()));
                    }
                    if !verified {
                        send(Event::Notice(crate::notifications::Kind::Warning, match result.outcome { Outcome::RestartRequired => "Restart mungkin diperlukan untuk menyelesaikan uninstall.", Outcome::Cancelled => "Pemantauan uninstall dibatalkan.", _ => "Proses uninstall selesai, tetapi status penghapusan belum dapat dipastikan. Segarkan untuk memeriksa kembali." }.into()));
                    }
                    send(Event::Pending(history.pending_restart.len()));
                    Ok(match result.outcome {Outcome::Removed=>"Aplikasi berhasil dihapus dan penghapusannya terverifikasi.",Outcome::RestartRequired=>"Windows meminta restart. Status disimpan; segarkan daftar setelah restart.",Outcome::Cancelled=>"Pemantauan dibatalkan. Uninstaller eksternal dapat tetap berjalan; tidak ada pembersihan otomatis.",Outcome::Unverified=>"Uninstaller selesai, tetapi penghapusan belum dapat dipastikan. Tidak ada pembersihan otomatis."}.into())
                }
                Job::Scan { registry } => {
                    progress("Mencari sisa aplikasi dari riwayat uninstall terverifikasi…");
                    let inv = inventory()?;
                    let scan =
                        cleanup::scan(&history.removed, &inv, &roots, registry, &cancel, |n| {
                            if n % 16 == 0 {
                                progress(&format!("Memindai file sisa… {n} item"));
                            }
                        })?;
                    let count = scan.candidates.len();
                    send(Event::Scan(scan));
                    send(Event::Inventory(inv, Vec::new()));
                    send(Event::Quarantine(Quarantine::entries(&roots)?));
                    Ok(format!(
                        "Pemindaian selesai: {count} kandidat. Belum ada file yang diubah."
                    ))
                }
                Job::Clean {
                    candidates,
                    ids,
                    dry_run,
                } => {
                    let inv = inventory()?;
                    let result = Quarantine::clean(
                        &roots,
                        &inv,
                        &candidates,
                        &ids,
                        true,
                        dry_run,
                        &cancel,
                        |n| {
                            progress(&format!(
                                "{} {n}/{} item",
                                if dry_run { "Memeriksa" } else { "Mengarantina" },
                                ids.len()
                            ))
                        },
                    )?;
                    tracing::info!(
                        moved = result.moved,
                        bytes = result.bytes,
                        failed = result.failed.len(),
                        dry_run,
                        "Apps cleanup result"
                    );
                    send(Event::CleanupFailures(
                        result.failed.iter().take(64).cloned().collect(),
                    ));
                    if !dry_run {
                        match cleanup::scan(&history.removed, &inv, &roots, false, &cancel, |_| {})
                        {
                            Ok(scan) => send(Event::Scan(scan)),
                            Err(e) => {
                                tracing::warn!(detail=%e,"Post-cleanup scan did not complete; retaining prior results")
                            }
                        }
                    }
                    send(Event::Quarantine(Quarantine::entries(&roots)?));
                    if !result.failed.is_empty() {
                        progress(&format!(
                            "{} item tidak dapat dibersihkan. File yang gagal tetap di lokasi semula.",
                            result.failed.len()
                        ));
                        for e in &result.failed {
                            tracing::warn!(detail=%e,"Apps item skipped");
                        }
                    }
                    if !dry_run {
                        send(Event::Notice(
                            if result.failed.is_empty() {
                                crate::notifications::Kind::Success
                            } else {
                                crate::notifications::Kind::Warning
                            },
                            format!(
                                "{} item dikarantina; {} dilewati. File masih dapat dipulihkan.",
                                result.moved,
                                result.failed.len()
                            ),
                        ));
                    }
                    Ok(if dry_run {
                        format!(
                            "Pratinjau {}: {} item tervalidasi; {} gagal. Tidak ada file diubah.",
                            if result.cancelled {
                                "dibatalkan"
                            } else {
                                "selesai"
                            },
                            result.skipped,
                            result.failed.len()
                        )
                    } else {
                        format!(
                            "{} item ({}) dipindahkan ke karantina; {} gagal. Ruang disk belum dikosongkan karena file masih dapat dipulihkan.{}",
                            result.moved,
                            rigpulse_apps::size_label(Some(result.bytes)),
                            result.failed.len(),
                            if result.cancelled {
                                " Tugas dibatalkan pada batas aman."
                            } else {
                                ""
                            }
                        )
                    })
                }
                Job::Recover { id, purge } => {
                    Quarantine::recover(&roots, &id, true, purge)?;
                    tracing::info!(purge, "Apps recovery action complete");
                    send(Event::Quarantine(Quarantine::entries(&roots)?));
                    send(Event::Notice(
                        crate::notifications::Kind::Success,
                        if purge {
                            "Item karantina dihapus permanen"
                        } else {
                            "File berhasil dipulihkan"
                        }
                        .into(),
                    ));
                    Ok(if purge {
                        "Item karantina dihapus permanen."
                    } else {
                        "File berhasil dipulihkan ke jalur aslinya."
                    }
                    .into())
                }
            }
        })();
        match result {
            Ok(message) => send(Event::Done(message)),
            Err(e) => {
                let detail = format!("{e:#}");
                let permission = detail.contains("0x80070005")
                    || detail.contains("0x800702E4")
                    || detail.contains("os error 5")
                    || detail.contains("os error 740");
                tracing::warn!(detail=%detail,"Apps task failed");
                send(Event::Failed(detail, permission));
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verified_refresh_keeps_filters_closes_removed_details_and_emits_notifications() {
        let dir = std::env::temp_dir().join(format!("rigpulse-apps-events-{}", std::process::id()));
        let ctx = egui::Context::default();
        let mut c = Controller::new(dir);
        let (tx, rx) = mpsc::sync_channel(8);
        c.rx = rx;
        c.query = "Keep".into();
        c.selected = Some("gone".into());
        c.details_open = true;
        c.busy = true;
        let inv = Inventory {
            apps: vec![Application {
                id: "keep".into(),
                name: "Keep app".into(),
                ..Default::default()
            }],
            scanned_at: chrono::Utc::now(),
            warnings: Vec::new(),
            completed_sources: vec!["isolated fixture".into()],
        };
        tx.send(Event::Inventory(inv, Vec::new())).unwrap();
        tx.send(Event::Notice(
            crate::notifications::Kind::Success,
            "Gone berhasil di-uninstall".into(),
        ))
        .unwrap();
        tx.send(Event::Done("Selesai".into())).unwrap();
        c.poll(&ctx, &rigpulse_core::Settings::default());
        assert_eq!(c.query, "Keep");
        assert_eq!(c.filtered.len(), 1);
        assert_eq!(c.inventory.as_ref().unwrap().apps.len(), 1);
        assert!(c.selected.is_none());
        assert!(!c.details_open);
        assert!(!c.busy);
        assert_eq!(c.notifications[0].0, crate::notifications::Kind::Success);
        tx.send(Event::Failed("isolated permission failure".into(), true))
            .unwrap();
        c.poll(&ctx, &rigpulse_core::Settings::default());
        assert!(c.permission_denied);
        assert_eq!(
            c.notifications.last().unwrap().0,
            crate::notifications::Kind::Error
        );
    }
}
