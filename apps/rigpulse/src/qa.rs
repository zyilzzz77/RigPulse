use eframe::egui;
/// Opt-in native diagnostics; absent from normal production builds.
pub struct NativeProfile {
    path: Option<std::path::PathBuf>,
    created: std::time::Instant,
    active: bool,
    last: Option<std::time::Instant>,
    cpu: Vec<f64>,
    intervals: Vec<f64>,
}
impl NativeProfile {
    pub fn new() -> Self {
        Self {
            path: std::env::var_os("RIGPULSE_QA_PERF").map(Into::into),
            created: std::time::Instant::now(),
            active: false,
            last: None,
            cpu: Vec::new(),
            intervals: Vec::new(),
        }
    }
    pub fn input(&mut self, ctx: &egui::Context, input: &mut egui::RawInput, ready: bool) {
        if self.path.is_none() {
            return;
        }
        if ready && self.created.elapsed().as_secs() >= 10 {
            self.active = true;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        if self.active {
            let rect = input.screen_rect.unwrap_or_else(|| ctx.screen_rect());
            input.events.push(egui::Event::PointerMoved(egui::pos2(
                rect.width() * 0.65,
                rect.height() * 0.7,
            )));
            input.events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(
                    0.0,
                    if (self.cpu.len() / 12).is_multiple_of(2) {
                        -140.0
                    } else {
                        140.0
                    },
                ),
                modifiers: Default::default(),
            });
        }
    }
    pub fn record(&mut self, ctx: &egui::Context, cpu: std::time::Duration, counts: [usize; 3]) {
        if !self.active || self.path.is_none() {
            return;
        }
        let now = std::time::Instant::now();
        if let Some(last) = self.last.replace(now) {
            self.intervals.push((now - last).as_secs_f64() * 1000.0);
        }
        self.cpu.push(cpu.as_secs_f64() * 1000.0);
        ctx.request_repaint();
        if self.cpu.len() >= 180 {
            fn summary(values: &[f64]) -> serde_json::Value {
                let mut v = values.to_vec();
                v.sort_by(f64::total_cmp);
                serde_json::json!({"mean_ms":v.iter().sum::<f64>()/v.len() as f64,"p50_ms":v[(v.len()-1)/2],"p95_ms":v[(v.len()-1)*95/100],"max_ms":v.last()})
            }
            let result = serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"profile":if cfg!(debug_assertions){"debug"}else{"release"},"frames":self.cpu.len(),"real_data_counts":{"apps":counts[0],"drivers":counts[1],"hardware_properties":counts[2]},"ui_cpu":summary(&self.cpu),"frame_interval":summary(&self.intervals),"method":"Native eframe/OpenGL window, actual Windows data and monitoring, synthetic high-resolution wheel events; frame intervals include scheduling/presentation but are not a physical input latency measurement."});
            let path = self.path.take().unwrap();
            if let Err(e) =
                std::fs::write(path, serde_json::to_vec_pretty(&result).unwrap_or_default())
            {
                tracing::error!("Native profile write failed: {e}");
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
/// Native render capture is opt-in and compiled only for developer QA builds.
pub struct Capture {
    path: Option<std::path::PathBuf>,
    start: std::time::Instant,
    requested: bool,
}
impl Capture {
    pub fn new() -> Self {
        Self {
            path: std::env::var_os("RIGPULSE_QA_CAPTURE").map(Into::into),
            start: std::time::Instant::now(),
            requested: false,
        }
    }
    pub fn update(&mut self, ctx: &egui::Context) {
        let Some(path) = &self.path else {
            return;
        };
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| {
                if let egui::Event::Screenshot { image, .. } = e {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        if let Some(image) = image {
            let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Err(e) = image::save_buffer(
                path,
                &bytes,
                image.width() as u32,
                image.height() as u32,
                image::ColorType::Rgba8,
            ) {
                tracing::error!("QA capture failed: {e}");
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            self.path = None;
            return;
        }
        let delay = std::env::var("RIGPULSE_QA_DELAY")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(7)
            .clamp(1, 30);
        if !self.requested && self.start.elapsed() >= std::time::Duration::from_secs(delay) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            self.requested = true;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }
}
