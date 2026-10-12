use rigpulse_monitor::SensorProvider;
fn main() -> anyhow::Result<()> {
    let dir = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("Provide output directory"))?,
    );
    std::fs::create_dir_all(&dir)?;
    let mut sensors = rigpulse_monitor::SystemSensors::new();
    std::thread::sleep(std::time::Duration::from_secs(1));
    let run = rigpulse_diagnostics::scan(1);
    let mut db = rigpulse_database::Database::open(&dir.join("qa.db"))?;
    let id = db.save_scan(&run)?;
    anyhow::ensure!(
        db.scan(id)?.findings.len() == run.findings.len(),
        "Database roundtrip failed"
    );
    let report = rigpulse_reports::Report {
        app_version: env!("CARGO_PKG_VERSION").into(),
        exported_at: chrono::Utc::now(),
        sensitive_included: false,
        inventory: Some(rigpulse_windows::inventory::collect()?),
        battery: Some(rigpulse_windows::battery::read()?),
        devices: Some(rigpulse_windows::devices::enumerate()?),
        updates: None,
        monitoring: Some(sensors.sample()),
        diagnostics: Some(run),
    }
    .private(false);
    report.json(&dir.join("RigPulse-report.json"))?;
    report.pdf(&dir.join("RigPulse-report.pdf"))?;
    println!(
        "Real-device report and database roundtrip verified in {}",
        dir.display()
    );
    Ok(())
}
