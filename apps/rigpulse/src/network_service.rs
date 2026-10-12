use rigpulse_network::{Cancellation, ookla::SpeedStage, tools::*};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
};
pub enum Tool {
    Wifi(u64),
    Ping(String),
    Public,
    Speed(std::path::PathBuf),
    Whois(String),
}
enum Event {
    Progress(String),
    SpeedStage(SpeedStage),
    SpeedSample {
        download: bool,
        mbps: f64,
        progress: Option<f64>,
    },
    Done(Box<Result<ResultData, String>>),
}
enum ResultData {
    Wifi(u64, String, u32),
    Ping(PingResult),
    Public(PublicInfo),
    Speed(SpeedResult),
    Whois(WhoisBundle),
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TrafficUnit {
    #[default]
    Mbps,
    MBps,
    KBps,
    Bps,
}
impl TrafficUnit {
    pub const ALL: [Self; 4] = [Self::Mbps, Self::MBps, Self::KBps, Self::Bps];
    pub fn label(self) -> &'static str {
        match self {
            Self::Mbps => "Mbps",
            Self::MBps => "MB/s",
            Self::KBps => "KB/s",
            Self::Bps => "B/s",
        }
    }
    /// Convert bytes/s to the selected display value. Mbps uses decimal SI
    /// (8 bit per byte, 1_000_000); byte units use decimal prefixes to match
    /// the existing history totals.
    pub fn format(self, bytes_per_sec: f64) -> String {
        match self {
            Self::Mbps => format!("{:.2} Mbps", bytes_per_sec * 8.0 / 1_000_000.0),
            Self::MBps => format!("{:.2} MB/s", bytes_per_sec / 1_000_000.0),
            Self::KBps => format!("{:.1} KB/s", bytes_per_sec / 1000.0),
            Self::Bps => format!("{:.0} B/s", bytes_per_sec),
        }
    }
    pub fn plot_value(self, bytes_per_sec: f64) -> f64 {
        match self {
            Self::Mbps => bytes_per_sec * 8.0 / 1_000_000.0,
            Self::MBps => bytes_per_sec / 1_000_000.0,
            Self::KBps => bytes_per_sec / 1000.0,
            Self::Bps => bytes_per_sec,
        }
    }
    pub fn axis_label(self, v: f64) -> String {
        match self {
            Self::Mbps => format!("{v:.0} Mbps"),
            Self::MBps => format!("{v:.1} MB/s"),
            Self::KBps => format!("{v:.0} KB/s"),
            Self::Bps => format!("{v:.0} B/s"),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpeedUiStage {
    #[default]
    Idle,
    ConsentRequired,
    Preparing,
    Connecting,
    TestingPing,
    TestingDownload,
    TestingUpload,
    Finalizing,
    Completed,
    Failed,
    Cancelled,
}
impl SpeedUiStage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "Siap",
            Self::ConsentRequired => "Perlu persetujuan",
            Self::Preparing => "Menyiapkan pengujian...",
            Self::Connecting => "Menghubungi server tes...",
            Self::TestingPing => "Mengukur latensi...",
            Self::TestingDownload => "Mengukur download...",
            Self::TestingUpload => "Mengukur upload...",
            Self::Finalizing => "Menghitung hasil akhir...",
            Self::Completed => "Pengujian selesai",
            Self::Failed => "Pengujian gagal",
            Self::Cancelled => "Pengujian dibatalkan",
        }
    }
}
pub struct Controller {
    pub wifi: Option<(u64, String, u32)>,
    pub monitor: rigpulse_network::Monitor,
    pub tab: usize,
    pub paused: bool,
    pub frozen:
        Option<std::collections::VecDeque<(chrono::DateTime<chrono::Utc>, rigpulse_network::Rate)>>,
    pub bytes_units: bool,
    pub traffic_unit: TrafficUnit,
    pub busy: bool,
    pub busy_kind: Option<&'static str>,
    pub status: String,
    pub ping: Option<PingResult>,
    pub public: Option<PublicInfo>,
    pub speed: Option<SpeedResult>,
    pub ookla_agreement: bool,
    // Speedtest modal + gauge state. `speed_live` is the latest genuine
    // Ookla sample; `speed_display` is the animated needle value.
    pub speed_open: bool,
    pub speed_stage: SpeedUiStage,
    pub speed_live: Option<f64>,
    pub speed_display: f64,
    pub gauge_max: f64,
    pub speed_progress: Option<f64>,
    pub speed_is_download: bool,
    pub speed_error: Option<String>,
    // WHOIS / RDAP modal state.
    pub whois_open: bool,
    pub whois_tab: usize,
    pub whois: Option<WhoisBundle>,
    pub whois_query: String,
    pub notifications: Vec<(crate::notifications::Kind, String)>,
    rx: Option<Receiver<Event>>,
    cancel: Cancellation,
    last_lookup: Option<std::time::Instant>,
    last_whois: Option<std::time::Instant>,
    enabled: bool,
    preferred: Option<u64>,
}
impl Controller {
    pub fn new(settings: &rigpulse_core::Settings, ctx: eframe::egui::Context) -> Self {
        Self {
            wifi: None,
            monitor: rigpulse_network::Monitor::start(
                settings.network_enabled,
                settings.preferred_adapter,
                move || ctx.request_repaint(),
            ),
            tab: 0,
            paused: false,
            frozen: None,
            bytes_units: false,
            traffic_unit: TrafficUnit::Mbps,
            busy: false,
            busy_kind: None,
            status: String::new(),
            ping: None,
            public: None,
            speed: None,
            ookla_agreement: false,
            speed_open: false,
            speed_stage: SpeedUiStage::Idle,
            speed_live: None,
            speed_display: 0.0,
            gauge_max: 100.0,
            speed_progress: None,
            speed_is_download: true,
            speed_error: None,
            whois_open: false,
            whois_tab: 0,
            whois: None,
            whois_query: String::new(),
            notifications: Vec::new(),
            rx: None,
            cancel: Arc::new(AtomicBool::new(false)),
            last_lookup: None,
            last_whois: None,
            enabled: settings.network_enabled,
            preferred: settings.preferred_adapter,
        }
    }
    pub fn poll(&mut self, settings: &rigpulse_core::Settings) {
        if self.enabled != settings.network_enabled {
            self.enabled = settings.network_enabled;
            self.monitor
                .command(rigpulse_network::Command::Enabled(self.enabled));
        }
        if self.preferred != settings.preferred_adapter {
            self.preferred = settings.preferred_adapter;
            self.monitor
                .command(rigpulse_network::Command::Select(self.preferred));
            self.frozen = None;
            self.paused = false;
        }
        // Keep legacy checkbox in sync with the unit selector.
        self.bytes_units = self.traffic_unit != TrafficUnit::Mbps;
        self.monitor.poll();
        if let Some(rx) = &self.rx {
            for event in rx.try_iter() {
                match event {
                    Event::Progress(s) => self.status = s,
                    Event::SpeedStage(stage) => {
                        self.speed_stage = match stage {
                            SpeedStage::Preparing => SpeedUiStage::Preparing,
                            SpeedStage::Connecting => SpeedUiStage::Connecting,
                            SpeedStage::TestingPing => SpeedUiStage::TestingPing,
                            SpeedStage::TestingDownload => SpeedUiStage::TestingDownload,
                            SpeedStage::TestingUpload => SpeedUiStage::TestingUpload,
                            SpeedStage::Finalizing => SpeedUiStage::Finalizing,
                        };
                        if matches!(
                            stage,
                            SpeedStage::TestingDownload | SpeedStage::TestingUpload
                        ) {
                            let is_dl = matches!(stage, SpeedStage::TestingDownload);
                            if is_dl != self.speed_is_download {
                                self.speed_is_download = is_dl;
                                self.speed_live = None;
                                self.speed_display = 0.0;
                                self.speed_progress = None;
                            }
                        }
                    }
                    Event::SpeedSample {
                        download,
                        mbps,
                        progress,
                    } => {
                        if mbps.is_finite() && mbps >= 0.0 {
                            self.speed_is_download = download;
                            self.speed_live = Some(mbps);
                            self.speed_progress = progress;
                            self.gauge_max = stable_gauge_scale(self.gauge_max, mbps);
                            self.speed_stage = if download {
                                SpeedUiStage::TestingDownload
                            } else {
                                SpeedUiStage::TestingUpload
                            };
                        }
                    }
                    Event::Done(result) => {
                        let was_speed = self.busy_kind == Some("speed");
                        let result = *result;
                        self.busy = false;
                        self.busy_kind = None;
                        match result {
                            Ok(data) => {
                                let label = match data {
                                    ResultData::Wifi(id, ssid, signal) => {
                                        self.wifi = Some((id, ssid, signal));
                                        "Detail Wi-Fi diperbarui"
                                    }
                                    ResultData::Ping(p) => {
                                        self.ping = Some(p);
                                        "Pengujian ping selesai"
                                    }
                                    ResultData::Public(p) => {
                                        self.public = Some(p);
                                        "IP publik diperbarui"
                                    }
                                    ResultData::Speed(p) => {
                                        self.speed = Some(p);
                                        self.speed_stage = SpeedUiStage::Completed;
                                        self.speed_progress = Some(1.0);
                                        "Speedtest Ookla selesai"
                                    }
                                    ResultData::Whois(w) => {
                                        self.whois = Some(w);
                                        self.whois_open = true;
                                        "Detail WHOIS/RDAP diperbarui"
                                    }
                                };
                                self.status = label.into();
                                self.notifications
                                    .push((crate::notifications::Kind::Success, label.into()));
                            }
                            Err(e) => {
                                if was_speed {
                                    let cancelled = self.cancel.load(Ordering::Relaxed);
                                    self.speed_stage = if cancelled {
                                        SpeedUiStage::Cancelled
                                    } else {
                                        SpeedUiStage::Failed
                                    };
                                    self.speed_error = Some(e.clone());
                                }
                                self.status = e.clone();
                                self.notifications.push((
                                    if self.cancel.load(Ordering::Relaxed) {
                                        crate::notifications::Kind::Information
                                    } else {
                                        crate::notifications::Kind::Error
                                    },
                                    e,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    /// Frame-time needle animation toward the latest genuine sample.
    /// Returns the display value; never invents measurements.
    pub fn animate_gauge(&mut self, dt: f32) -> f64 {
        if matches!(
            self.speed_stage,
            SpeedUiStage::TestingDownload | SpeedUiStage::TestingUpload
        ) && let Some(target) = self.speed_live
        {
            let tau = 0.18_f64;
            let k = 1.0 - (-f64::from(dt.max(0.0)) / tau).exp();
            self.speed_display += (target - self.speed_display) * k.clamp(0.0, 1.0);
            if (target - self.speed_display).abs() < 0.05 {
                self.speed_display = target;
            }
        }
        self.speed_display
    }
    pub fn begin_speedtest(&mut self) {
        self.speed_live = None;
        self.speed_display = 0.0;
        self.speed_progress = None;
        self.speed_error = None;
        self.speed_is_download = true;
        self.gauge_max = 100.0;
        self.speed_stage = SpeedUiStage::Preparing;
    }
    pub fn start(
        &mut self,
        tool: Tool,
        settings: &rigpulse_core::Settings,
        ctx: &eframe::egui::Context,
    ) {
        if self.busy {
            return;
        }
        if matches!(tool, Tool::Public) {
            if !settings.external_lookup {
                self.status =
                    "Aktifkan izin pencarian IP publik di Settings terlebih dahulu".into();
                return;
            }
            if self
                .last_lookup
                .is_some_and(|at| at.elapsed().as_secs() < 3600)
                && self.public.is_some()
            {
                self.status = "Hasil disimpan 1 jam untuk membatasi permintaan ke penyedia".into();
                return;
            }
            self.last_lookup = Some(std::time::Instant::now());
        }
        if let Tool::Whois(ref q) = tool {
            if !settings.external_lookup {
                self.status = "Izinkan pencarian jaringan eksternal terlebih dahulu".into();
                return;
            }
            if !valid_lookup_input(q)
                && self.public.as_ref().map(|p| p.ip.as_str()) != Some(q.as_str())
            {
                self.status = "Masukkan alamat IP publik yang valid".into();
                return;
            }
            if self
                .last_whois
                .is_some_and(|at| at.elapsed().as_secs() < 60)
                && self.whois.is_some()
            {
                self.whois_open = true;
                return;
            }
            self.last_whois = Some(std::time::Instant::now());
        }
        if matches!(tool, Tool::Speed(_)) && !self.ookla_agreement {
            self.speed_stage = SpeedUiStage::ConsentRequired;
            return;
        }
        self.cancel = Arc::new(AtomicBool::new(false));
        self.busy = true;
        self.busy_kind = Some(match tool {
            Tool::Wifi(_) => "wifi",
            Tool::Ping(_) => "ping",
            Tool::Public => "public",
            Tool::Speed(_) => "speed",
            Tool::Whois(_) => "whois",
        });
        self.status = "Menyiapkan pengujian…".into();
        if matches!(tool, Tool::Speed(_)) {
            self.begin_speedtest();
        }
        let cancel = self.cancel.clone();
        let ctx = ctx.clone();
        let (tx, rx) = mpsc::sync_channel(64);
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let progress = |s: String| {
                let _ = tx.try_send(Event::Progress(s));
                ctx.request_repaint();
            };
            let speed_event = |ev: rigpulse_network::ookla::SpeedEvent| {
                use rigpulse_network::ookla::SpeedEvent as E;
                match ev {
                    E::Stage(s) => {
                        let _ = tx.try_send(Event::SpeedStage(s));
                    }
                    E::DownloadSample { mbps, progress } => {
                        let _ = tx.try_send(Event::SpeedSample {
                            download: true,
                            mbps,
                            progress,
                        });
                    }
                    E::UploadSample { mbps, progress } => {
                        let _ = tx.try_send(Event::SpeedSample {
                            download: false,
                            mbps,
                            progress,
                        });
                    }
                    E::Message(m) => {
                        let _ = tx.try_send(Event::Progress(m));
                    }
                }
                ctx.request_repaint();
            };
            let result = match tool {
                Tool::Wifi(id) => rigpulse_network::native::wifi(id)
                    .map(|(ssid, signal)| ResultData::Wifi(id, ssid, signal)),
                Tool::Ping(target) => ping(&target, &cancel, progress).map(ResultData::Ping),
                Tool::Public => public_info(&cancel).map(ResultData::Public),
                Tool::Speed(dir) => {
                    rigpulse_network::ookla::run_with_events(&dir, &cancel, speed_event)
                        .map(ResultData::Speed)
                }
                Tool::Whois(query) => whois_bundle(&query, &cancel).map(ResultData::Whois),
            }
            .map_err(|e| e.to_string());
            let _ = tx.send(Event::Done(Box::new(result)));
            ctx.request_repaint();
        });
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.cancel();
    }
}
