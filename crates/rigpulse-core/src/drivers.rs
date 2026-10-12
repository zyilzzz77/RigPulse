use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceStatus {
    Working,
    Issue,
    MissingDriver,
    Disabled,
    Unknown,
}
pub fn device_status(problem: Option<u32>, started: bool) -> DeviceStatus {
    match problem {
        Some(22) => DeviceStatus::Disabled,
        Some(28) => DeviceStatus::MissingDriver,
        Some(0) if started => DeviceStatus::Working,
        Some(0) | None => DeviceStatus::Unknown,
        Some(_) => DeviceStatus::Issue,
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceDriverInfo {
    pub name: String,
    pub class: String,
    pub manufacturer: Option<String>,
    pub provider: Option<String>,
    pub version: Option<String>,
    pub date: Option<String>,
    pub status: DeviceStatus,
    pub problem: Option<u32>,
    pub instance_id: String,
    pub hardware_ids: Vec<String>,
}
impl DeviceDriverInfo {
    pub fn private(&self, sensitive: bool) -> Self {
        let mut d = self.clone();
        if !sensitive {
            d.instance_id.clear();
            d.hardware_ids.clear();
        }
        d
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverUpdate {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub max_download_bytes: Option<f64>,
    #[serde(default)]
    pub restart_possible: Option<bool>,
    #[serde(default)]
    pub license_accepted: bool,
    pub id: String,
    pub revision: i32,
    pub title: String,
    pub class: String,
    pub model: String,
    pub provider: String,
    pub hardware_id: String,
    pub downloaded: bool,
    pub eligible: bool,
    pub reason: Option<String>,
}
impl DriverUpdate {
    pub fn private(&self, sensitive: bool) -> Self {
        let mut u = self.clone();
        if !sensitive {
            u.hardware_id.clear();
        }
        u
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSearch {
    pub at: chrono::DateTime<chrono::Utc>,
    pub source: String,
    pub updates: Vec<DriverUpdate>,
    pub warning: Option<String>,
}
pub fn class_allowed(class: &str, hardware_id: &str, title: &str) -> bool {
    let text = format!("{class} {hardware_id} {title}").to_ascii_lowercase();
    if ["firmware", "bios", "uefi", "res_"]
        .iter()
        .any(|s| text.contains(s))
    {
        return false;
    }
    [
        "display",
        "net",
        "media",
        "bluetooth",
        "usb",
        "hidclass",
        "printer",
        "image",
        "mouse",
        "keyboard",
        "monitor",
        "audioendpoint",
    ]
    .contains(&class.to_ascii_lowercase().as_str())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalize_status() {
        assert_eq!(device_status(Some(28), false), DeviceStatus::MissingDriver);
        assert_eq!(device_status(Some(22), false), DeviceStatus::Disabled);
        assert_eq!(device_status(Some(43), false), DeviceStatus::Issue);
        assert_eq!(device_status(Some(9999), false), DeviceStatus::Issue);
        assert_eq!(device_status(None, false), DeviceStatus::Unknown);
        assert_eq!(device_status(Some(0), true), DeviceStatus::Working);
    }
    #[test]
    fn excludes_firmware() {
        assert!(class_allowed("Display", "PCI\\VEN_10DE", "Graphics driver"));
        assert!(!class_allowed("Firmware", "UEFI\\RES_123", "Update"));
        assert!(!class_allowed("System", "ACPI", "BIOS update"));
        assert!(!class_allowed("", "", "Unknown"));
    }
}
