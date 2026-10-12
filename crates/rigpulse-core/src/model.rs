use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Availability {
    Loading,
    Available,
    Unsupported,
    PermissionDenied,
    NotDetected,
    DriverMissing,
    TemporarilyUnavailable,
    Error,
    Stale,
}
impl Availability {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Loading => "Membaca data",
            Self::Available => "Terbaca",
            Self::Unsupported => "Belum didukung",
            Self::PermissionDenied => "Akses dibatasi",
            Self::NotDetected => "Tidak terdeteksi",
            Self::DriverMissing => "Driver tidak tersedia",
            Self::TemporarilyUnavailable => "Belum tersedia",
            Self::Error => "Gagal membaca",
            Self::Stale => "Data lama",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurement {
    pub value: Option<f64>,
    pub unit: String,
    pub label: String,
    pub source: String,
    pub at: DateTime<Utc>,
    pub availability: Availability,
    pub reason: Option<String>,
}
impl Measurement {
    pub fn available(label: &str, value: f64, unit: &str, source: &str) -> Self {
        if !value.is_finite() {
            return Self::unavailable(
                label,
                unit,
                source,
                Availability::Error,
                "Provider mengembalikan nilai tidak valid",
            );
        }
        Self {
            value: Some(value),
            unit: unit.into(),
            label: label.into(),
            source: source.into(),
            at: Utc::now(),
            availability: Availability::Available,
            reason: None,
        }
    }
    pub fn unavailable(
        label: &str,
        unit: &str,
        source: &str,
        availability: Availability,
        reason: &str,
    ) -> Self {
        Self {
            value: None,
            unit: unit.into(),
            label: label.into(),
            source: source.into(),
            at: Utc::now(),
            availability,
            reason: Some(reason.into()),
        }
    }
    pub fn stale(&self, now: DateTime<Utc>, seconds: i64) -> bool {
        (now - self.at).num_seconds() > seconds
    }
    pub fn text(&self) -> String {
        self.value
            .map(|v| format!("{v:.1} {}", self.unit))
            .unwrap_or_else(|| "Tidak tersedia".into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Property {
    pub name: String,
    pub value: String,
    pub sensitive: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareSection {
    pub name: String,
    pub source: String,
    pub properties: Vec<Property>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    pub at: DateTime<Utc>,
    pub sections: Vec<HardwareSection>,
}
impl Inventory {
    pub fn private(&self, include_sensitive: bool) -> Self {
        let mut copy = self.clone();
        if !include_sensitive {
            for s in &mut copy.sections {
                s.properties.retain(|p| !p.sensitive);
            }
        }
        copy
    }
    pub fn specifications(&self, sensitive: bool) -> String {
        let mut out = format!(
            "RigPulse — Spesifikasi perangkat\nWaktu UTC: {}\n",
            self.at.to_rfc3339()
        );
        for s in self.private(sensitive).sections {
            out.push_str(&format!("\n{}\n", s.name));
            if let Some(e) = s.error {
                out.push_str(&format!("Tidak tersedia: {e}\n"));
            }
            for p in s.properties {
                out.push_str(&format!("{}: {}\n", p.name, p.value));
            }
        }
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryPack {
    #[serde(default)]
    pub watts: Option<f64>,
    #[serde(default)]
    pub voltage: Option<f64>,
    #[serde(default)]
    pub chemistry: Option<String>,
    #[serde(default)]
    pub power_state: Option<u32>,
    #[serde(default)]
    pub manufacturer: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    pub design_mwh: Option<u32>,
    pub full_mwh: Option<u32>,
    pub cycles: Option<u32>,
    pub relative: bool,
}
impl BatteryPack {
    pub fn health(&self) -> Option<f64> {
        battery_health(self.design_mwh, self.full_mwh, self.relative)
    }
}
/// BATTERY_STATUS rate is signed mW; relative-capacity packs have no absolute power unit.
pub fn battery_watts(rate: i32, state: u32, relative: bool) -> Option<f64> {
    if relative || rate == i32::MIN || state & 6 == 6 {
        return None;
    }
    if (rate > 0 && state & 4 == 0) || (rate < 0 && state & 2 == 0) {
        return None;
    }
    Some(f64::from(rate) / 1000.0)
}
pub fn battery_health(design: Option<u32>, full: Option<u32>, relative: bool) -> Option<f64> {
    if relative {
        return None;
    }
    let (d, f) = (design?, full?);
    if d == 0 || f == 0 || d == u32::MAX || f == u32::MAX {
        return None;
    }
    Some(f64::from(f) / f64::from(d) * 100.0)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryReading {
    pub at: DateTime<Utc>,
    pub present: Option<bool>,
    pub percent: Option<u8>,
    pub ac: Option<bool>,
    pub charging: Option<bool>,
    pub remaining_seconds: Option<u32>,
    pub packs: Vec<BatteryPack>,
    pub note: Option<String>,
}
impl BatteryReading {
    pub fn watts(&self) -> Option<f64> {
        if self.present != Some(true) || self.packs.is_empty() {
            return None;
        }
        self.packs
            .iter()
            .try_fold(0.0, |sum, pack| pack.watts.map(|w| sum + w))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn battery_validates_units_and_unknowns() {
        assert_eq!(battery_watts(0, 0, false), Some(0.0));
        assert_eq!(battery_watts(100, 6, false), None);
        assert_eq!(battery_watts(23800, 4, false), Some(23.8));
        assert_eq!(battery_watts(-18400, 2, false), Some(-18.4));
        assert_eq!(battery_watts(i32::MIN, 4, false), None);
        assert_eq!(battery_watts(100, 4, true), None);
        assert_eq!(battery_watts(-100, 4, false), None);
        assert_eq!(battery_health(Some(50000), Some(40000), false), Some(80.0));
        assert_eq!(battery_health(Some(0), Some(1), false), None);
        assert_eq!(battery_health(None, Some(1), false), None);
        assert_eq!(battery_health(Some(10), Some(5), true), None);
        assert_eq!(battery_health(Some(u32::MAX), Some(1), false), None);
        assert!((battery_health(Some(10), Some(11), false).unwrap() - 110.0).abs() < 1e-10);
    }
    #[test]
    fn battery_aggregate_requires_complete_real_readings() {
        let mut b = BatteryReading {
            at: Utc::now(),
            present: Some(true),
            percent: Some(100),
            ac: Some(true),
            charging: Some(false),
            remaining_seconds: None,
            packs: Vec::new(),
            note: None,
        };
        assert_eq!(b.watts(), None);
        let pack = BatteryPack {
            manufacturer: None,
            model: None,
            design_mwh: None,
            full_mwh: None,
            cycles: None,
            relative: false,
            watts: Some(10.0),
            voltage: None,
            chemistry: None,
            power_state: Some(4),
        };
        b.packs.push(pack.clone());
        b.packs.push(BatteryPack {
            watts: Some(-2.0),
            ..pack
        });
        assert_eq!(b.watts(), Some(8.0));
        b.packs[1].watts = None;
        assert_eq!(b.watts(), None);
        b.present = Some(false);
        assert_eq!(b.watts(), None);
    }
    #[test]
    fn privacy_removes_identifiers() {
        let i = Inventory {
            at: Utc::now(),
            sections: vec![HardwareSection {
                name: "BIOS".into(),
                source: "test".into(),
                error: None,
                properties: vec![
                    Property {
                        name: "SerialNumber".into(),
                        value: "secret".into(),
                        sensitive: true,
                    },
                    Property {
                        name: "Version".into(),
                        value: "1".into(),
                        sensitive: false,
                    },
                ],
            }],
        };
        assert!(!i.specifications(false).contains("secret"));
        assert!(i.specifications(true).contains("secret"));
    }
    #[test]
    fn invalid_measurement_is_not_zero() {
        let m = Measurement::available("CPU", f64::NAN, "%", "test");
        assert!(m.value.is_none());
        assert_eq!(m.availability, Availability::Error);
    }
    #[test]
    fn stale_detection() {
        let m = Measurement::available("CPU", 12.0, "%", "test");
        assert!(m.stale(Utc::now() + chrono::Duration::seconds(30), 10));
    }
}
