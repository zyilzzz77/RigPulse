use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppKind {
    #[default]
    Desktop,
    Msi,
    Store,
}
impl AppKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Desktop => "Desktop",
            Self::Msi => "Windows Installer (MSI)",
            Self::Store => "Microsoft Store / MSIX",
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Application {
    pub id: String,
    pub name: String,
    pub publisher: Option<String>,
    pub version: Option<String>,
    pub installed: Option<chrono::NaiveDate>,
    pub estimated_bytes: Option<u64>,
    pub location: Option<PathBuf>,
    pub kind: AppKind,
    pub protected: bool,
    pub source: String,
    pub uninstall: Option<String>,
    pub product_code: Option<String>,
    pub package_family: Option<String>,
    pub icon_path: Option<PathBuf>,
}
impl Application {
    pub fn removable(&self) -> bool {
        !self.protected
            && self.name.to_lowercase() != "rigpulse"
            && (self.uninstall.is_some()
                || self.product_code.is_some()
                || self.kind == AppKind::Store)
    }
    pub fn same_family(&self, other: &Self) -> bool {
        self.id == other.id
            || (self.kind == AppKind::Store
                && self.package_family.is_some()
                && self.package_family == other.package_family)
            || (self.name.eq_ignore_ascii_case(&other.name) && self.publisher == other.publisher)
    }
    pub fn details(&self) -> String {
        format!(
            "{}\nPublisher: {}\nVersi: {}\nDipasang: {}\nTerakhir digunakan: Tidak tersedia\nUkuran: {}\nLokasi: {}\nJenis: {}\nSumber: {}",
            self.name,
            self.publisher.as_deref().unwrap_or("Tidak tersedia"),
            self.version.as_deref().unwrap_or("Tidak tersedia"),
            self.installed
                .map(|d| d.format("%d/%m/%Y").to_string())
                .unwrap_or_else(|| "Tidak tersedia".into()),
            size_label(self.estimated_bytes),
            self.location
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "Tidak tersedia".into()),
            self.kind.label(),
            self.source
        )
    }
}
#[derive(Debug, Clone)]
pub struct Inventory {
    pub apps: Vec<Application>,
    pub scanned_at: chrono::DateTime<chrono::Utc>,
    pub warnings: Vec<String>,
    pub completed_sources: Vec<String>,
}
pub fn size_label(value: Option<u64>) -> String {
    value
        .map(|b| {
            if b >= 1024 * 1024 * 1024 {
                format!("{:.2} GiB", b as f64 / 1073741824.0)
            } else {
                format!("{:.1} MiB", b as f64 / 1048576.0)
            }
        })
        .unwrap_or_else(|| "Tidak tersedia".into())
}
