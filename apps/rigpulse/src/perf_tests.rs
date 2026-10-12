//! Development-only CPU frame profiling through the application's render/input boundary.
//! Fixtures never enter a production executable. Real sensor sampling stays active.
use super::*;
use rigpulse_core::{DeviceDriverInfo, DeviceStatus, HardwareSection, Inventory, Property};
use serde_json::json;
use std::time::Instant;

fn fixture(ctx: &egui::Context, dir: std::path::PathBuf) -> RigPulseApp {
    let mut app = RigPulseApp::from_context(ctx, dir);
    // Do not let asynchronous startup inventory replace the deliberately large test corpus.
    // Monitoring retains its real provider/channel and keeps running during all frames.
    app.rx = std::sync::mpsc::sync_channel(16).1;
    app.busy = false;
    app.notice = None;
    app.settings.auto_check_updates = false;
    app.devices = Some(
        (0..500)
            .map(|n| DeviceDriverInfo {
                name: format!("Fixture Driver {n:04} with a long device name"),
                class: "Display".into(),
                manufacturer: Some("Fixture manufacturer".into()),
                provider: Some("Fixture provider".into()),
                version: Some("32.0.15.0000".into()),
                date: None,
                status: DeviceStatus::Working,
                problem: Some(0),
                instance_id: format!("fixture-{n}"),
                hardware_ids: vec!["test-only".into()],
            })
            .collect(),
    );
    app.apps.inventory = Some(rigpulse_apps::Inventory {
        apps: (0..1000)
            .map(|n| rigpulse_apps::Application {
                id: format!("fixture-{n}"),
                name: format!("Fixture App {n:04} with a long application name"),
                publisher: Some("Fixture Publisher".into()),
                version: Some("1.2.3".into()),
                estimated_bytes: Some(128 * 1024 * 1024),
                ..Default::default()
            })
            .collect(),
        scanned_at: chrono::Utc::now(),
        warnings: vec![],
        completed_sources: vec!["fixture".into()],
    });
    app.inventory = Some(Inventory { at: chrono::Utc::now(), sections: (0..10).map(|section| HardwareSection {
        name: format!("Fixture Hardware Section {section}"), source: "Test fixture only".into(), error: None,
        properties: (0..100).map(|n| Property { name: format!("Fixture property {n:03}"),
            value: format!("Property {section}/{n}: a long value which must wrap correctly at narrow widths without losing data"), sensitive: false }).collect(),
    }).collect() });
    app.scans = (0..200)
        .map(|n| rigpulse_database::HistoryEntry {
            id: n,
            at: "2026-10-11T12:00:00Z".into(),
            kind: "diagnostics".into(),
            status: "Completed".into(),
        })
        .collect();
    let start = chrono::Utc::now() - chrono::Duration::seconds(3600);
    for n in 0..3600 {
        app.history.push(
            start + chrono::Duration::seconds(n),
            (n % 100) as f64,
            (n % 60 + 20) as f64,
        );
    }
    app
}
fn percentile(values: &mut [f64], percent: usize) -> f64 {
    values.sort_by(f64::total_cmp);
    values[((values.len() - 1) * percent / 100).min(values.len() - 1)]
}
#[test]
#[ignore = "Development CPU profiling: fixture corpus, real background monitoring; writes a JSON report"]
fn scroll_frame_profile() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/performance-phase1");
    std::fs::create_dir_all(&root).unwrap();
    let label = std::env::var("RIGPULSE_PERF_LABEL").unwrap_or_else(|_| "manual".into());
    assert!(label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    let ctx = egui::Context::default();
    let mut app = fixture(&ctx, root.join("profile-data"));
    let mut results = Vec::new();
    let mut clock = 0.0;
    let frames = std::env::var("RIGPULSE_PERF_FRAMES")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(40)
        .clamp(10, 300);
    for (width, height) in [
        (1280.0, 720.0),
        (1366.0, 768.0),
        (1920.0, 1080.0),
        (2560.0, 1440.0),
    ] {
        for scale in [1.0, 1.25, 1.5, 1.75] {
            for (name, page, selected) in [
                ("Apps", Page::Apps, false),
                ("AppsDetails", Page::Apps, true),
                ("Drivers", Page::Drivers, false),
                ("Hardware", Page::Hardware, false),
                ("Monitoring", Page::Monitoring, false),
                ("Reports", Page::Reports, false),
            ] {
                app.page = page;
                app.apps.selected = selected.then(|| "fixture-0".into());
                app.apps.details_open = false;
                let mut durations = Vec::new();
                let mut layout = 0.0;
                let mut tessellation = 0.0;
                let mut vertices = 0usize;
                for frame in 0..frames + 8 {
                    clock += 1.0 / 60.0;
                    let mut input = egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width / scale, height / scale),
                        )),
                        time: Some(clock),
                        ..Default::default()
                    };
                    input
                        .viewports
                        .get_mut(&egui::ViewportId::ROOT)
                        .unwrap()
                        .native_pixels_per_point = Some(scale);
                    input.events = vec![
                        egui::Event::PointerMoved(egui::pos2(
                            width / scale * 0.65,
                            height / scale * 0.7,
                        )),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(
                                0.0,
                                if (frame / 12) % 2 == 0 { -180.0 } else { 180.0 },
                            ),
                            modifiers: Default::default(),
                        },
                    ];
                    let start = Instant::now();
                    let output = ctx.run(input, |ctx| app.render(ctx));
                    let laid_out = Instant::now();
                    let primitives = ctx.tessellate(output.shapes, scale);
                    if frame >= 8 {
                        layout += (laid_out - start).as_secs_f64() * 1000.0;
                        tessellation += laid_out.elapsed().as_secs_f64() * 1000.0;
                        durations.push(start.elapsed().as_secs_f64() * 1000.0);
                        vertices = primitives
                            .iter()
                            .filter_map(|p| {
                                if let egui::epaint::Primitive::Mesh(m) = &p.primitive {
                                    Some(m.vertices.len())
                                } else {
                                    None
                                }
                            })
                            .sum();
                    }
                }
                let mean = durations.iter().sum::<f64>() / frames as f64;
                let p50 = percentile(&mut durations, 50);
                let p95 = percentile(&mut durations, 95);
                let max = *durations.last().unwrap();
                results.push(json!({"page":name,"physical_size":[width,height],"dpi":scale,"frames":frames,"mean_ms":mean,"p50_ms":p50,"p95_ms":p95,"max_ms":max,"layout_mean_ms":layout/frames as f64,"tessellation_mean_ms":tessellation/frames as f64,"last_frame_vertices":vertices}));
            }
        }
    }
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let report = json!({"label":label,"profile":profile,"corpus":{"apps":1000,"drivers":500,"hardware_properties":1000,"reports":200,"history_samples":3600},"measurement":"egui CPU render/layout + tessellation, after 8 warm-up frames; real monitoring worker active. Not GPU/present latency or physical input certification.","results":results});
    let path = root.join(format!("{label}-{profile}.json"));
    std::fs::write(&path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("Profile report: {}", path.display());
}
