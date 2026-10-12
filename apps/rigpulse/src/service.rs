use rigpulse_core::{BatteryReading, Inventory};
use std::sync::mpsc::SyncSender;

pub enum Event {
    Inventory(Inventory),
    Battery(BatteryReading),
    Error(String),
    ScanDone,
    Devices(Vec<rigpulse_core::DeviceDriverInfo>),
    Updates(Result<rigpulse_core::UpdateSearch, String>),
    Diagnostics(rigpulse_core::DiagnosticRun),
    History(Vec<rigpulse_database::HistoryEntry>),
    Export(Result<std::path::PathBuf, String>),
    HistoryDeleted(Result<(), String>),
    HistoryLoaded(Result<rigpulse_core::DiagnosticRun, String>),
    InstallProgress(rigpulse_drivers::install::Progress),
    SearchProgress(rigpulse_drivers::install::Progress),
    AdminClosed(Result<(), String>),
    InstallDone(Result<rigpulse_drivers::install::InstallationResult, String>),
}
pub fn scan(tx: SyncSender<Event>, ctx: eframe::egui::Context) {
    std::thread::spawn(move || {
        match rigpulse_windows::inventory::collect() {
            Ok(i) => {
                let _ = tx.send(Event::Inventory(i));
            }
            Err(e) => {
                let _ = tx.send(Event::Error(format!("Inventory: {e}")));
            }
        }
        ctx.request_repaint();
        match rigpulse_windows::battery::read() {
            Ok(b) => {
                let _ = tx.send(Event::Battery(b));
            }
            Err(e) => {
                let _ = tx.send(Event::Error(format!("Battery: {e}")));
            }
        }
        match rigpulse_windows::devices::enumerate() {
            Ok(d) => {
                let _ = tx.send(Event::Devices(d));
            }
            Err(e) => {
                let _ = tx.send(Event::Error(format!("SetupAPI: {e}")));
            }
        }
        let _ = tx.send(Event::ScanDone);
        ctx.request_repaint();
    });
}
pub fn open_admin(tx: SyncSender<Event>, ctx: eframe::egui::Context) {
    std::thread::spawn(move || {
        let result = rigpulse_drivers::privilege::open_manager().map_err(|e| e.to_string());
        let _ = tx.send(Event::AdminClosed(result));
        ctx.request_repaint();
    });
}
pub fn search(
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    tx: SyncSender<Event>,
    ctx: eframe::egui::Context,
) {
    std::thread::spawn(move || {
        let result=rigpulse_drivers::wua::search_with(cancel,|p|{let _=tx.try_send(Event::SearchProgress(p));ctx.request_repaint();}).map_err(|e|format!("Windows Update tidak dapat diperiksa: {e}. Kebijakan organisasi atau koneksi dapat membatasi pencarian."));
        let _ = tx.send(Event::Updates(result));
        ctx.request_repaint();
    });
}
pub fn diagnose(
    days: u32,
    dir: std::path::PathBuf,
    tx: SyncSender<Event>,
    ctx: eframe::egui::Context,
) {
    std::thread::spawn(move || {
        let run = rigpulse_diagnostics::scan(days);
        let stored = (|| -> anyhow::Result<()> {
            let mut db = rigpulse_database::Database::open(&dir.join("rigpulse.db"))?;
            db.save_scan(&run)?;
            if let Ok(b) = rigpulse_windows::battery::read() {
                db.save_battery(&b)?;
            }
            let entries = db.history()?;
            let _ = tx.send(Event::History(entries));
            Ok(())
        })();
        if let Err(e) = stored {
            let _ = tx.send(Event::Error(format!(
                "Hasil scan tersedia, tetapi riwayat gagal disimpan: {e}"
            )));
        }
        let _ = tx.send(Event::Diagnostics(run));
        ctx.request_repaint();
    });
}
pub fn load_history(dir: std::path::PathBuf, tx: SyncSender<Event>, ctx: eframe::egui::Context) {
    std::thread::spawn(move || {
        match rigpulse_database::Database::open(&dir.join("rigpulse.db"))
            .and_then(|db| db.history())
        {
            Ok(h) => {
                let _ = tx.send(Event::History(h));
            }
            Err(e) => {
                let _ = tx.send(Event::Error(format!("Riwayat tidak tersedia: {e}")));
            }
        }
        ctx.request_repaint();
    });
}
pub fn load_scan(
    id: i64,
    dir: std::path::PathBuf,
    tx: SyncSender<Event>,
    ctx: eframe::egui::Context,
) {
    std::thread::spawn(move || {
        let result = rigpulse_database::Database::open(&dir.join("rigpulse.db"))
            .and_then(|db| db.scan(id))
            .map_err(|e| e.to_string());
        let _ = tx.send(Event::HistoryLoaded(result));
        ctx.request_repaint();
    });
}
pub fn delete_history(dir: std::path::PathBuf, tx: SyncSender<Event>, ctx: eframe::egui::Context) {
    std::thread::spawn(move || {
        let result = rigpulse_database::Database::open(&dir.join("rigpulse.db"))
            .and_then(|mut db| db.delete_history())
            .map_err(|e| e.to_string());
        let _ = tx.send(Event::HistoryDeleted(result));
        ctx.request_repaint();
    });
}
pub fn export(
    report: rigpulse_reports::Report,
    pdf: bool,
    dir: std::path::PathBuf,
    tx: SyncSender<Event>,
    ctx: eframe::egui::Context,
) {
    std::thread::spawn(move || {
        let path = dir.join("reports").join(format!(
            "RigPulse-{}.{}",
            chrono::Utc::now().format("%Y%m%d-%H%M%S-%f"),
            if pdf { "pdf" } else { "json" }
        ));
        let result = (if pdf {
            report.pdf(&path)
        } else {
            report.json(&path)
        })
        .and_then(|()| {
            anyhow::ensure!(std::fs::metadata(&path)?.len() > 0, "File kosong");
            Ok(path)
        })
        .map_err(|e| format!("Ekspor gagal: {e}"));
        let _ = tx.send(Event::Export(result));
        ctx.request_repaint();
    });
}
pub fn install(
    consent: rigpulse_drivers::install::Consent,
    download_only: bool,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    dir: std::path::PathBuf,
    tx: SyncSender<Event>,
    ctx: eframe::egui::Context,
) {
    std::thread::spawn(move || {
        let result = (|| -> anyhow::Result<rigpulse_drivers::install::InstallationResult> {
            let db = rigpulse_database::Database::open(&dir.join("rigpulse.db"))?;
            let operation = db.begin_driver_operation(consent.package_count())?;
            let result =
                rigpulse_drivers::install::execute(consent, download_only, cancel, |state| {
                    let _ = tx.try_send(Event::InstallProgress(state));
                    ctx.request_repaint();
                });
            let (state, error, restart) = match &result {
                Ok(r) => (
                    format!("{:?}", r.state),
                    None,
                    r.state == rigpulse_drivers::install::InstallState::RestartRequired
                        || r.packages.iter().any(|p| p.restart_required),
                ),
                Err(e) => ("Failed".into(), Some(e.to_string()), false),
            };
            if let Err(e) = db.finish_driver_operation(operation, &state, error.as_deref(), restart)
            {
                tracing::error!("Driver audit completion failed: {e}");
                let _ = tx.send(Event::Error(format!(
                    "Operasi selesai tetapi pencatatan akhir gagal: {e}"
                )));
            }
            result
        })()
        .map_err(|e| e.to_string());
        let _ = tx.send(Event::InstallDone(result));
        ctx.request_repaint();
    });
}
