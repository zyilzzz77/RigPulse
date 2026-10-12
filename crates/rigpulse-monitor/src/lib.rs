pub mod buffer;
#[cfg(test)]
use chrono::Utc;
use rigpulse_core::{Availability, BatteryReading, Measurement};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver},
    },
    time::Duration,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub cpu: Measurement,
    pub memory: Measurement,
    pub extra: Vec<Measurement>,
    pub battery: Option<BatteryReading>,
}
pub trait SensorProvider {
    fn sample(&mut self) -> Sample;
}
pub struct SystemSensors {
    system: sysinfo::System,
    nvml: Option<nvml_wrapper::Nvml>,
    gpu_reason: String,
    #[cfg(windows)]
    windows_gpu: Result<rigpulse_windows::gpu::GpuCounters, String>,
    last_battery: Option<BatteryReading>,
    last_battery_poll: std::time::Instant,
}
impl Default for SystemSensors {
    fn default() -> Self {
        Self::new()
    }
}
impl SystemSensors {
    pub fn new() -> Self {
        let mut system = sysinfo::System::new();
        system.refresh_cpu_usage();
        system.refresh_memory();
        // Load the optional vendor DLL only from trusted absolute installation paths.
        let mut nvml = None;
        #[cfg(windows)]
        for path in [
            std::env::var_os("WINDIR")
                .map(|p| std::path::PathBuf::from(p).join("System32/nvml.dll")),
            std::env::var_os("ProgramW6432")
                .map(|p| std::path::PathBuf::from(p).join("NVIDIA Corporation/NVSMI/nvml.dll")),
        ]
        .into_iter()
        .flatten()
        {
            if path.is_file()
                && let Ok(n) = nvml_wrapper::Nvml::builder()
                    .lib_path(path.as_os_str())
                    .init()
            {
                nvml = Some(n);
                break;
            }
        }
        Self {
            system,
            nvml,
            gpu_reason: "NVIDIA NVML tidak tersedia. Dukungan bergantung GPU dan driver.".into(),
            #[cfg(windows)]
            windows_gpu: rigpulse_windows::gpu::GpuCounters::new().map_err(|e| e.to_string()),
            last_battery: None,
            last_battery_poll: std::time::Instant::now() - Duration::from_secs(6),
        }
    }
}
impl SensorProvider for SystemSensors {
    fn sample(&mut self) -> Sample {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        let cpu = f64::from(self.system.global_cpu_usage());
        let cpu = if self.system.cpus().is_empty() || !(0.0..=100.0).contains(&cpu) {
            Measurement::unavailable(
                "CPU",
                "%",
                "sysinfo / Windows",
                Availability::TemporarilyUnavailable,
                "Counter CPU belum tersedia",
            )
        } else {
            Measurement::available("CPU", cpu, "%", "sysinfo / Windows")
        };
        let total = self.system.total_memory();
        let used = self.system.used_memory();
        let memory = if total == 0 || used > total {
            Measurement::unavailable(
                "RAM",
                "%",
                "sysinfo / Windows",
                Availability::Error,
                "Counter memori tidak valid",
            )
        } else {
            Measurement::available(
                "RAM",
                used as f64 / total as f64 * 100.0,
                "%",
                "sysinfo / Windows",
            )
        };
        let mut extra = vec![
            Measurement::unavailable(
                "Suhu CPU",
                "°C",
                "Sensor hardware",
                Availability::Unsupported,
                "Provider suhu CPU terverifikasi belum tersedia; thermal zone ACPI bukan suhu inti CPU.",
            ),
            Measurement::unavailable(
                "Fan",
                "RPM",
                "Sensor hardware",
                Availability::Unsupported,
                "RPM fan tidak diekspos oleh provider yang didukung.",
            ),
            Measurement::unavailable(
                "Aktivitas disk",
                "%",
                "Windows counters",
                Availability::Unsupported,
                "Provider counter aktivitas disk belum tersedia.",
            ),
        ];
        #[cfg(windows)]
        match &mut self.windows_gpu {
            Ok(gpu) => extra.extend(gpu.sample()),
            Err(e) => extra.push(Measurement::unavailable(
                "GPU Windows",
                "%",
                "Windows DXGI / WDDM",
                Availability::Unsupported,
                e,
            )),
        }
        if let Some(nvml) = &self.nvml {
            match nvml.device_count() {
                Ok(count) if count > 0 => {
                    for n in 0..count {
                        match nvml.device_by_index(n) {
                            Ok(gpu) => {
                                let name = gpu.name().unwrap_or_else(|_| format!("GPU {n}"));
                                for (suffix, unit, value) in [
                                    (
                                        "daya",
                                        "W",
                                        gpu.power_usage().map(|v| f64::from(v) / 1000.0),
                                    ),
                                    (
                                        "clock grafis",
                                        "GHz",
                                        gpu.clock_info(
                                            nvml_wrapper::enum_wrappers::device::Clock::Graphics,
                                        )
                                        .map(|v| f64::from(v) / 1000.0),
                                    ),
                                    (
                                        "kecepatan kipas GPU",
                                        "RPM",
                                        gpu.fan_speed_rpm(0).map(f64::from),
                                    ),
                                ] {
                                    extra.push(match value {
                                        Ok(v) => Measurement::available(
                                            &format!("{name} {suffix}"),
                                            v,
                                            unit,
                                            "NVIDIA NVML",
                                        ),
                                        Err(e) => Measurement::unavailable(
                                            &format!("{name} {suffix}"),
                                            unit,
                                            "NVIDIA NVML",
                                            Availability::Unsupported,
                                            &e.to_string(),
                                        ),
                                    });
                                }
                                extra.push(match gpu.utilization_rates() {
                                    Ok(v) => Measurement::available(
                                        &format!("{name} usage"),
                                        f64::from(v.gpu),
                                        "%",
                                        "NVIDIA NVML",
                                    ),
                                    Err(e) => Measurement::unavailable(
                                        &format!("{name} usage"),
                                        "%",
                                        "NVIDIA NVML",
                                        Availability::TemporarilyUnavailable,
                                        &e.to_string(),
                                    ),
                                });
                                extra.push(
                                    match gpu.temperature(
                                        nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu,
                                    ) {
                                        Ok(v) if v < 200 => Measurement::available(
                                            &format!("{name} suhu"),
                                            f64::from(v),
                                            "°C",
                                            "NVIDIA NVML",
                                        ),
                                        Ok(_) => Measurement::unavailable(
                                            &format!("{name} suhu"),
                                            "°C",
                                            "NVIDIA NVML",
                                            Availability::Error,
                                            "Pembacaan sensor tidak valid",
                                        ),
                                        Err(e) => Measurement::unavailable(
                                            &format!("{name} suhu"),
                                            "°C",
                                            "NVIDIA NVML",
                                            Availability::TemporarilyUnavailable,
                                            &e.to_string(),
                                        ),
                                    },
                                );
                                if let Ok(m) = gpu.memory_info() {
                                    extra.push(Measurement::available(
                                        &format!("{name} VRAM digunakan"),
                                        m.used as f64 / 1048576.0,
                                        "MiB",
                                        "NVIDIA NVML",
                                    ));
                                    extra.push(Measurement::available(
                                        &format!("{name} VRAM total"),
                                        m.total as f64 / 1048576.0,
                                        "MiB",
                                        "NVIDIA NVML",
                                    ));
                                }
                            }
                            Err(e) => extra.push(Measurement::unavailable(
                                "GPU",
                                "%",
                                "NVIDIA NVML",
                                Availability::TemporarilyUnavailable,
                                &e.to_string(),
                            )),
                        }
                    }
                }
                _ => extra.push(Measurement::unavailable(
                    "GPU",
                    "%",
                    "NVIDIA NVML",
                    Availability::NotDetected,
                    "Tidak ada GPU NVIDIA yang dapat dibaca",
                )),
            }
        } else {
            extra.push(Measurement::unavailable(
                "GPU",
                "%",
                "NVIDIA NVML",
                Availability::DriverMissing,
                &self.gpu_reason,
            ));
        }
        if self.last_battery_poll.elapsed() >= Duration::from_secs(5) {
            self.last_battery = rigpulse_windows::battery::read().ok();
            self.last_battery_poll = std::time::Instant::now();
        }
        Sample {
            cpu,
            memory,
            extra,
            battery: self.last_battery.clone(),
        }
    }
}

pub struct Monitor {
    pub rx: Receiver<Sample>,
    interval: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    wake: std::sync::mpsc::Sender<()>,
}
impl Monitor {
    pub fn start(power_saver: bool, repaint: impl Fn() + Send + 'static) -> Self {
        let (tx, rx) = mpsc::sync_channel(2);
        let (wake, wake_rx) = mpsc::channel();
        let interval = Arc::new(AtomicU64::new(if power_saver { 5000 } else { 1000 }));
        let stop = Arc::new(AtomicBool::new(false));
        let (i, s) = (interval.clone(), stop.clone());
        std::thread::spawn(move || {
            let mut provider = SystemSensors::new();
            loop {
                let _ = wake_rx.recv_timeout(Duration::from_millis(i.load(Ordering::Relaxed)));
                if s.load(Ordering::Relaxed) {
                    break;
                }
                let sample = provider.sample();
                match tx.try_send(sample) {
                    Ok(()) => repaint(),
                    Err(mpsc::TrySendError::Full(_)) => {}
                    Err(mpsc::TrySendError::Disconnected(_)) => break,
                }
            }
        });
        Self {
            rx,
            interval,
            stop,
            wake,
        }
    }
    pub fn set_power_saver(&self, on: bool) {
        self.set_interval_seconds(if on { 5 } else { 1 });
    }
    pub fn set_interval_seconds(&self, seconds: u64) {
        self.interval
            .store(seconds.clamp(1, 5) * 1000, Ordering::Relaxed);
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.wake.send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_cpu_memory_samples() {
        let mut p = SystemSensors::new();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL + Duration::from_millis(100));
        let s = p.sample();
        assert!(s.cpu.value.is_some_and(|v| (0.0..=100.0).contains(&v)));
        assert!(s.memory.value.is_some_and(|v| (0.0..=100.0).contains(&v)));
        assert_eq!(s.cpu.at.date_naive(), Utc::now().date_naive());
    }
}
