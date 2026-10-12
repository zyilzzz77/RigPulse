#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod apps_service;
mod network_service;
mod notifications;
mod pages;
#[cfg(feature = "qa-screenshot")]
mod qa;
mod service;
mod theme;
mod updater;
mod virtual_rows;

fn main() {
    if let Err(error) = run() {
        tracing::error!("Startup gagal: {error}");
        show_error(
            "RigPulse tidak dapat dibuka. Periksa folder data atau dukungan grafis Windows.",
        );
    }
}
fn run() -> anyhow::Result<()> {
    let dir = rigpulse_core::data_dir()?;
    std::fs::create_dir_all(dir.join("logs"))?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("logs/rigpulse.log"))?;
    tracing_subscriber::fmt()
        .json()
        .with_writer(std::sync::Mutex::new(file))
        .with_max_level(tracing::Level::INFO)
        .init();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "RigPulse startup");
    let logo =
        image::load_from_memory(include_bytes!("../../../RigPulse Neon Monitoring Logo.png"))?
            .resize(256, 256, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
    let icon = eframe::egui::IconData {
        width: logo.width(),
        height: logo.height(),
        rgba: logo.into_raw(),
    };
    let size = [1200.0, 780.0];
    #[cfg(feature = "qa-screenshot")]
    let size = std::env::var("RIGPULSE_QA_SIZE")
        .ok()
        .and_then(|s| {
            s.split_once('x')
                .and_then(|(w, h)| Some([w.parse().ok()?, h.parse().ok()?]))
        })
        .unwrap_or(size);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_icon(std::sync::Arc::new(icon))
            .with_inner_size(size)
            .with_min_inner_size([620.0, 400.0]),
        ..Default::default()
    };
    eframe::run_native(
        "RigPulse",
        options,
        Box::new(move |cc| Ok(Box::new(app::RigPulseApp::new(cc, dir)))),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))
}
fn show_error(message: &str) {
    #[cfg(windows)]
    unsafe {
        use windows::{
            Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW},
            core::PCWSTR,
        };
        let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
        MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            windows::core::w!("RigPulse"),
            MB_OK | MB_ICONERROR,
        );
    }
    #[cfg(not(windows))]
    eprintln!("{message}");
}
