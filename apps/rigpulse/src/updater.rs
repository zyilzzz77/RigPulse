use rigpulse_updater::{InstallOutcome, Progress, ReleaseConfig, VerifiedPackage, VerifiedRelease};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

#[derive(Clone, Debug)]
pub enum State {
    Idle,
    Unconfigured,
    Checking,
    Current,
    Available(VerifiedRelease),
    Downloading(u64, u64),
    Verifying,
    Ready(VerifiedPackage),
    Installing,
    Installed(bool),
    Cancelled,
    Failed(String),
}
pub struct Controller {
    pub notifications: Vec<(crate::notifications::Kind, String)>,
    pub state: State,
    pub confirm: bool,
    attempted: bool,
    config: Option<ReleaseConfig>,
    tx: mpsc::SyncSender<State>,
    rx: mpsc::Receiver<State>,
    cancel: Arc<AtomicBool>,
}
impl Controller {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel(16);
        let (config, state) = match ReleaseConfig::bundled() {
            Ok(Some(c)) => (Some(c), State::Idle),
            Ok(None) => (None, State::Unconfigured),
            Err(e) => (None, State::Failed(e.to_string())),
        };
        Self {
            notifications: Vec::new(),
            state,
            confirm: false,
            attempted: false,
            config,
            tx,
            rx,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn busy(&self) -> bool {
        matches!(
            self.state,
            State::Checking | State::Downloading(..) | State::Verifying | State::Installing
        )
    }
    pub fn poll(&mut self) {
        while let Ok(state) = self.rx.try_recv() {
            use crate::notifications::Kind;
            let notice = match &state {
                State::Current => Some((Kind::Information, "Versi aplikasi sudah terbaru".into())),
                State::Available(_) => {
                    Some((Kind::Information, "Pembaruan RigPulse tersedia".into()))
                }
                State::Ready(_) => Some((
                    Kind::Success,
                    "Paket pembaruan terverifikasi dan siap dipasang".into(),
                )),
                State::Installed(restart) => Some((
                    if *restart {
                        Kind::Warning
                    } else {
                        Kind::Success
                    },
                    if *restart {
                        "Installer selesai; Windows meminta restart"
                    } else {
                        "Installer selesai"
                    }
                    .into(),
                )),
                State::Failed(e) => Some((Kind::Error, e.clone())),
                State::Cancelled => Some((Kind::Information, "Pembaruan dibatalkan".into())),
                _ => None,
            };
            if let Some(notice) = notice {
                self.notifications.push(notice);
            }
            self.state = state;
        }
    }
    pub fn automatic(
        &mut self,
        settings: &mut rigpulse_core::Settings,
        dir: &std::path::Path,
        ctx: &eframe::egui::Context,
    ) {
        self.poll();
        let due = settings
            .last_app_update_check
            .is_none_or(|at| chrono::Utc::now() - at >= chrono::Duration::hours(24));
        if settings.auto_check_updates && !self.attempted && due && !self.busy() {
            self.check(settings, dir, ctx);
        }
    }
    pub fn check(
        &mut self,
        settings: &mut rigpulse_core::Settings,
        dir: &std::path::Path,
        ctx: &eframe::egui::Context,
    ) {
        if self.busy() {
            return;
        }
        self.attempted = true;
        let Some(config) = self.config.clone() else {
            self.state = State::Unconfigured;
            return;
        };
        settings.last_app_update_check = Some(chrono::Utc::now());
        if let Err(e) = settings.save(dir) {
            self.state = State::Failed(format!("Waktu pemeriksaan gagal disimpan: {e}"));
            return;
        }
        self.state = State::Checking;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let state = match rigpulse_updater::check(&config, env!("CARGO_PKG_VERSION")) {
                Ok(Some(r)) => State::Available(r),
                Ok(None) => State::Current,
                Err(e) => State::Failed(format!("{e:#}")),
            };
            let _ = tx.send(state);
            ctx.request_repaint();
        });
    }
    pub fn download(&mut self, dir: &std::path::Path, ctx: &eframe::egui::Context) {
        let State::Available(release) = &self.state else {
            return;
        };
        let release = release.clone();
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        self.state = State::Downloading(0, release.metadata().package.size);
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let dir = dir.join("updates");
        std::thread::spawn(move || {
            let result = rigpulse_updater::download(release, &dir, &cancel, |p| {
                let _ = tx.try_send(match p {
                    Progress::Downloading(n, total) => State::Downloading(n, total),
                    Progress::Verifying => State::Verifying,
                });
                ctx.request_repaint();
            });
            let state = match result {
                Ok(p) => State::Ready(p),
                Err(_) if cancel.load(Ordering::Relaxed) => State::Cancelled,
                Err(e) => State::Failed(format!("{e:#}")),
            };
            let _ = tx.send(state);
            ctx.request_repaint();
        });
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
    pub fn install(&mut self, ctx: &eframe::egui::Context) {
        let State::Ready(p) = &self.state else {
            return;
        };
        let p = p.clone();
        self.confirm = false;
        self.state = State::Installing;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let state = match rigpulse_updater::install(&p, true) {
                Ok(InstallOutcome::Completed) => State::Installed(false),
                Ok(InstallOutcome::RestartRequired) => State::Installed(true),
                Ok(InstallOutcome::Cancelled) => State::Cancelled,
                Err(e) => State::Failed(format!("{e:#}")),
            };
            let _ = tx.send(state);
            ctx.request_repaint();
        });
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.cancel();
    }
}
