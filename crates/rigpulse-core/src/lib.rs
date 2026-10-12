use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
pub mod diagnostics;
pub mod drivers;
pub use diagnostics::*;
pub mod model;
pub use drivers::*;
pub use model::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Dashboard,
    Hardware,
    Monitoring,
    Drivers,
    Apps,
    Battery,
    Diagnostics,
    Reports,
    Settings,
}
impl Page {
    pub const ALL: [Self; 9] = [
        Self::Dashboard,
        Self::Hardware,
        Self::Monitoring,
        Self::Drivers,
        Self::Apps,
        Self::Battery,
        Self::Diagnostics,
        Self::Reports,
        Self::Settings,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Hardware => "Hardware",
            Self::Monitoring => "Monitoring",
            Self::Drivers => "Drivers",
            Self::Apps => "Apps",
            Self::Battery => "Battery",
            Self::Diagnostics => "Diagnostics",
            Self::Reports => "Reports",
            Self::Settings => "Settings",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Dashboard => "Ringkasan kondisi perangkat Anda.",
            Self::Hardware => "Spesifikasi perangkat, langsung dari Windows.",
            Self::Monitoring => "Penggunaan perangkat secara langsung beserta sumber datanya.",
            Self::Drivers => "Kelola perangkat dan periksa pembaruan driver resmi.",
            Self::Apps => "Kelola aplikasi, hapus software, dan bersihkan sisa instalasi.",
            Self::Battery => "Daya, kapasitas, dan perkiraan kesehatan baterai.",
            Self::Diagnostics => "Pemeriksaan berbasis bukti tanpa mengubah sistem.",
            Self::Reports => "Riwayat pemeriksaan dan laporan lokal Anda.",
            Self::Settings => "Preferensi aplikasi dan privasi data lokal.",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub network_enabled: bool,
    pub network_history_minutes: i64,
    pub preferred_adapter: Option<u64>,
    pub ping_target: String,
    pub external_lookup: bool,
    pub driver_notifications: bool,
    pub power_saver: bool,
    pub include_sensitive: bool,
    pub refresh_seconds: u64,
    pub history_minutes: i64,
    pub reduced_animations: bool,
    pub interface_scale: f32,
    pub auto_check_updates: bool,
    pub last_app_update_check: Option<chrono::DateTime<chrono::Utc>>,
    pub apps_auto_scan: bool,
    pub apps_background_scan: bool,
    pub apps_preserve_data: bool,
    pub apps_registry_review: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            network_enabled: true,
            network_history_minutes: 5,
            preferred_adapter: None,
            ping_target: "1.1.1.1".into(),
            external_lookup: false,
            driver_notifications: true,
            power_saver: false,
            include_sensitive: false,
            refresh_seconds: 1,
            history_minutes: 5,
            reduced_animations: false,
            interface_scale: 1.0,
            auto_check_updates: true,
            last_app_update_check: None,
            apps_auto_scan: true,
            apps_background_scan: false,
            apps_preserve_data: true,
            apps_registry_review: false,
        }
    }
}

pub fn data_dir() -> anyhow::Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var_os("LOCALAPPDATA")
            .ok_or_else(|| anyhow::anyhow!("LOCALAPPDATA tidak tersedia"))?,
    )
    .join("RigPulse"))
}
impl Settings {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let path = dir.join("settings.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let mut settings: Self = serde_json::from_slice(&std::fs::read(path)?)?;
        settings.normalize();
        Ok(settings)
    }
    pub fn save(&self, dir: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join("settings.json"), serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }
    pub fn normalize(&mut self) {
        if ![5, 15, 60].contains(&self.network_history_minutes) {
            self.network_history_minutes = 5;
        }
        if self.ping_target.len() > 253 || self.ping_target.trim().is_empty() {
            self.ping_target = "1.1.1.1".into();
        }
        if ![1, 3, 5].contains(&self.refresh_seconds) {
            self.refresh_seconds = 1;
        }
        if ![5, 15, 60].contains(&self.history_minutes) {
            self.history_minutes = 5;
        }
        if !self.interface_scale.is_finite() || !(0.8..=1.75).contains(&self.interface_scale) {
            self.interface_scale = 1.0;
        }
    }
    pub fn interval_seconds(&self) -> u64 {
        if self.power_saver {
            self.refresh_seconds.max(5)
        } else {
            self.refresh_seconds
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_is_complete() {
        assert_eq!(Page::ALL.len(), 9);
        let mut labels: Vec<_> = Page::ALL.iter().map(|p| p.title()).collect();
        labels.sort();
        labels.dedup();
        assert_eq!(labels.len(), 9);
    }
    #[test]
    fn privacy_defaults_off() {
        let s = Settings::default();
        assert!(!s.include_sensitive);
        assert!(!s.power_saver);
    }
    #[test]
    fn settings_migrate_validate_and_persist() {
        let dir = std::env::temp_dir().join(format!("rigpulse-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("settings.json"),
            r#"{"power_saver":true,"include_sensitive":false}"#,
        )
        .unwrap();
        let mut settings = Settings::load(&dir).unwrap();
        assert!(settings.auto_check_updates);
        assert!(settings.apps_auto_scan);
        assert!(settings.apps_preserve_data);
        assert!(!settings.apps_background_scan);
        assert!(!settings.apps_registry_review);
        assert!(settings.network_enabled);
        assert!(!settings.external_lookup);
        assert_eq!(settings.preferred_adapter, None);
        assert_eq!(settings.network_history_minutes, 5);
        assert_eq!(settings.interval_seconds(), 5);
        settings.refresh_seconds = 3;
        settings.history_minutes = 60;
        settings.reduced_animations = true;
        settings.interface_scale = 1.25;
        settings.apps_background_scan = true;
        settings.apps_registry_review = true;
        settings.apps_preserve_data = false;
        settings.preferred_adapter = Some(42);
        settings.network_history_minutes = 15;
        settings.ping_target = "::1".into();
        settings.external_lookup = true;
        settings.driver_notifications = false;
        settings.save(&dir).unwrap();
        let read = Settings::load(&dir).unwrap();
        assert_eq!(read.refresh_seconds, 3);
        assert_eq!(read.history_minutes, 60);
        assert_eq!(read.interface_scale, 1.25);
        assert!(read.reduced_animations);
        assert!(read.apps_background_scan);
        assert!(read.apps_registry_review);
        assert!(!read.apps_preserve_data);
        assert_eq!(read.preferred_adapter, Some(42));
        assert_eq!(read.network_history_minutes, 15);
        assert!(read.external_lookup);
        assert!(!read.driver_notifications);
        assert_eq!(read.ping_target, "::1");
        settings.interface_scale = f32::NAN;
        settings.refresh_seconds = 0;
        settings.history_minutes = -1;
        settings.normalize();
        assert_eq!(settings.interface_scale, 1.0);
        assert_eq!(settings.refresh_seconds, 1);
        assert_eq!(settings.history_minutes, 5);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
