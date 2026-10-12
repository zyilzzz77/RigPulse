//! Presentation of Windows inventory values with explicit, human-readable units.
pub fn property(key: &str, raw: &str) -> (String, String) {
    let label = match key {
        "Name" => "Nama",
        "Manufacturer" => "Produsen",
        "Model" => "Model perangkat",
        "SystemType" => "Jenis sistem",
        "TotalPhysicalMemory" => "RAM yang dapat digunakan",
        "NumberOfCores" => "Inti prosesor",
        "NumberOfLogicalProcessors" => "Proses logis (thread)",
        "MaxClockSpeed" => "Kecepatan nominal (Windows)",
        "CurrentClockSpeed" => "Kecepatan dilaporkan Windows",
        "VideoProcessor" => "Chip grafis",
        "DriverVersion" => "Versi driver",
        "CurrentHorizontalResolution" => "Lebar layar",
        "CurrentVerticalResolution" => "Tinggi layar",
        "CurrentRefreshRate" => "Refresh layar",
        "PartNumber" => "Model komponen",
        "Capacity" => "Kapasitas",
        "Speed" => "Kecepatan RAM",
        "ConfiguredClockSpeed" => "Kecepatan RAM terpasang",
        "DeviceLocator" => "Slot RAM",
        "Product" => "Model motherboard",
        "Version" => "Versi",
        "SerialNumber" => "Nomor seri",
        "SMBIOSBIOSVersion" => "Versi BIOS",
        "ReleaseDate" => "Tanggal BIOS",
        "ChassisTypes" => "Jenis casing (kode Windows)",
        "Size" => "Kapasitas penyimpanan",
        "InterfaceType" => "Koneksi",
        "MediaType" => "Jenis media",
        "FirmwareRevision" => "Versi firmware",
        "NetConnectionStatus" => "Status koneksi",
        "PhysicalAdapter" => "Adaptor fisik",
        "Status" => "Status perangkat",
        "Caption" => "Edisi Windows",
        "BuildNumber" => "Build Windows",
        "OSArchitecture" => "Arsitektur",
        "LastBootUpTime" => "Terakhir dinyalakan",
        _ => key,
    };
    if raw == "Tidak tersedia" {
        return (label.into(), raw.into());
    }
    let number = raw
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0);
    let value = match key {
        "MaxClockSpeed" | "CurrentClockSpeed" => number
            .filter(|v| *v > 0.0)
            .map(|v| format!("{:.2} GHz", v / 1000.0))
            .unwrap_or_else(|| "Tidak tersedia".into()),
        "TotalPhysicalMemory" | "Capacity" => number
            .map(|v| format!("{:.2} GiB", v / 1073741824.0))
            .unwrap_or_else(|| "Tidak tersedia".into()),
        "Size" => number
            .map(|v| format!("{:.1} GB", v / 1_000_000_000.0))
            .unwrap_or_else(|| "Tidak tersedia".into()),
        "Speed" | "ConfiguredClockSpeed" => number
            .filter(|v| *v > 0.0)
            .map(|v| format!("{v:.0} MHz (laporan firmware)"))
            .unwrap_or_else(|| "Tidak tersedia".into()),
        "CurrentRefreshRate" => number
            .filter(|v| *v > 1.0)
            .map(|v| format!("{v:.0} Hz"))
            .unwrap_or_else(|| "Tidak tersedia".into()),
        "CurrentHorizontalResolution" | "CurrentVerticalResolution" => number
            .filter(|v| *v > 0.0)
            .map(|v| format!("{v:.0} piksel"))
            .unwrap_or_else(|| "Tidak tersedia".into()),
        "NumberOfCores" => format!("{raw} inti"),
        "NumberOfLogicalProcessors" => format!("{raw} thread"),
        "NetConnectionStatus" => match raw {
            "0" => "Terputus",
            "1" => "Menghubungkan",
            "2" => "Terhubung",
            "4" => "Perangkat tidak ada",
            "5" => "Dinonaktifkan",
            "7" => "Kabel terlepas",
            _ => "Status lain / tidak diketahui",
        }
        .into(),
        "PhysicalAdapter" => match raw {
            "true" => "Ya",
            "false" => "Virtual / perangkat lunak",
            _ => "Tidak tersedia",
        }
        .into(),
        "Status" if raw == "OK" => "Berfungsi (laporan Windows)".into(),
        "ReleaseDate" | "LastBootUpTime" => format_date(raw),
        _ => raw.into(),
    };
    (label.into(), value)
}
fn format_date(raw: &str) -> String {
    if let Ok(date) = chrono::DateTime::parse_from_rfc3339(raw) {
        return date.format("%d/%m/%Y %H:%M").to_string();
    }
    if raw.len() >= 14 && raw.as_bytes()[..14].iter().all(u8::is_ascii_digit) {
        return format!(
            "{}/{}/{} {}:{} (waktu Windows)",
            &raw[6..8],
            &raw[4..6],
            &raw[..4],
            &raw[8..10],
            &raw[10..12]
        );
    }
    raw.into()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn units_and_missing_values_are_readable() {
        assert_eq!(property("MaxClockSpeed", "3100").1, "3.10 GHz");
        assert_eq!(property("Capacity", "8589934592").1, "8.00 GiB");
        assert_eq!(property("Size", "1000000000000").1, "1000.0 GB");
        assert_eq!(property("CurrentClockSpeed", "0").1, "Tidak tersedia");
        assert_eq!(property("Capacity", "Tidak tersedia").1, "Tidak tersedia");
        assert_eq!(property("NumberOfCores", "8").1, "8 inti");
    }
}
