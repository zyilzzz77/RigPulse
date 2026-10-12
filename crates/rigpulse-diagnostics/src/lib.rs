use chrono::Utc;
use rigpulse_core::{DiagnosticRun, Finding, FindingStatus as Status, Severity};
fn finding(
    category: &str,
    severity: Severity,
    status: Status,
    title: &str,
    explanation: &str,
    evidence: &str,
    action: &str,
) -> Finding {
    Finding {
        category: category.into(),
        severity,
        status,
        title: title.into(),
        explanation: explanation.into(),
        evidence: evidence.into(),
        recommended_action: action.into(),
        at: Utc::now(),
        can_auto_fix: false,
    }
}
pub fn scan(days: u32) -> DiagnosticRun {
    let started_at = Utc::now();
    let mut findings = Vec::new();
    match rigpulse_windows::devices::enumerate() {
        Ok(devices) => {
            let mut count = 0;
            for d in devices {
                if let Some(code) = d.problem
                    && code != 0
                {
                    count += 1;
                    findings.push(finding("Perangkat",Severity::Warning,Status::Detected,&d.name,"Windows melaporkan masalah atau perangkat dinonaktifkan. Ini tidak membuktikan pembaruan driver akan memperbaikinya.",&format!("Configuration Manager problem code {code}; kelas {}",d.class),"Periksa detail perangkat dan Device Manager."));
                }
            }
            if count == 0 {
                findings.push(finding(
                    "Perangkat",
                    Severity::Info,
                    Status::NotDetected,
                    "Tidak ditemukan problem code",
                    "Hanya berlaku pada device node yang berhasil dibaca.",
                    "SetupAPI / CM_Get_DevNode_Status",
                    "Tidak diperlukan tindakan berdasarkan pemeriksaan ini.",
                ));
            }
        }
        Err(e) => findings.push(finding(
            "Perangkat",
            Severity::Info,
            Status::Unavailable,
            "Pemeriksaan perangkat tidak tersedia",
            "Enumerasi perangkat gagal.",
            &e.to_string(),
            "Coba scan kembali.",
        )),
    }
    match rigpulse_windows::event_log::recent_errors(days) {
        Ok(events) => {
            let count = events.len();
            if events.is_empty() {
                findings.push(finding(
                    "Event Log",
                    Severity::Info,
                    Status::NotDetected,
                    "Tidak ditemukan event level Error/Critical",
                    "Pencarian dibatasi log System dan rentang waktu yang dipilih.",
                    &format!("Windows Event Log; {days} hari"),
                    "Tidak diperlukan tindakan berdasarkan pemeriksaan ini.",
                ));
            }
            for e in events {
                findings.push(finding("Event Log",Severity::Warning,Status::Detected,&format!("{} / Event {}",e.provider,e.id),"Windows mencatat event Error/Critical. Event historis tidak selalu menunjukkan kerusakan hardware yang masih berlangsung.",&format!("System; provider {}; EventID {}; level {}; UTC {}; rentang {days} hari; maksimal 100 event terbaru ({count} terbaca)",e.provider,e.id,e.level,e.time),"Buka Event Viewer untuk memeriksa konteks dan pengulangan event."));
            }
        }
        Err(e) => findings.push(finding(
            "Event Log",
            Severity::Info,
            Status::Unavailable,
            "Event Log tidak tersedia",
            "Izin atau layanan Windows membatasi pembacaan.",
            &e.to_string(),
            "Coba scan kembali atau periksa layanan Event Log.",
        )),
    }
    match rigpulse_windows::battery::read() {
        Ok(b) if b.present == Some(false) => findings.push(finding(
            "Baterai",
            Severity::Info,
            Status::NotDetected,
            "Tidak ada baterai terdeteksi",
            "Sistem tidak melaporkan baterai.",
            "Windows GetSystemPowerStatus",
            "Tidak diperlukan pada PC desktop.",
        )),
        Ok(b) => {
            let valid: Vec<_> = b
                .packs
                .iter()
                .filter_map(|p| p.health().map(|h| (p, h)))
                .collect();
            if valid.is_empty() {
                findings.push(finding(
                    "Baterai",
                    Severity::Info,
                    Status::Unavailable,
                    "Kesehatan baterai tidak dapat dihitung",
                    "Kapasitas valid dan satuan mWh yang sama diperlukan.",
                    "Windows Battery IOCTL",
                    "Periksa dukungan driver baterai.",
                ));
            }
            for (p, h) in valid {
                findings.push(finding("Baterai",Severity::Info,Status::NotDetected,&format!("Perkiraan kapasitas baterai {h:.1}%"),"Rasio kapasitas penuh terhadap desain, bukan diagnosis universal kondisi baterai.",&format!("Battery IOCTL; kapasitas awal {}; kapasitas penuh {}",p.design_mwh.map(|v|format!("{:.2} Wh",f64::from(v)/1000.0)).unwrap_or_else(||"Tidak tersedia".into()),p.full_mwh.map(|v|format!("{:.2} Wh",f64::from(v)/1000.0)).unwrap_or_else(||"Tidak tersedia".into())),"Perhatikan perubahan kapasitas dan waktu penggunaan."));
            }
        }
        Err(e) => findings.push(finding(
            "Baterai",
            Severity::Info,
            Status::Unavailable,
            "Data baterai tidak tersedia",
            "Windows Battery API gagal.",
            &e.to_string(),
            "Coba kembali.",
        )),
    }
    storage(&mut findings);
    for (name, reason) in [
        (
            "SMART/NVMe",
            "Counter SMART/NVMe mentah belum diimplementasikan.",
        ),
        (
            "Suhu CPU/Fan",
            "Provider sensor terverifikasi belum tersedia.",
        ),
        (
            "Anomali penggunaan",
            "Diagnosis jendela penggunaan berkelanjutan belum diimplementasikan.",
        ),
    ] {
        findings.push(finding(
            name,
            Severity::Info,
            Status::Unavailable,
            "Pemeriksaan belum tersedia",
            reason,
            "Capability detection RigPulse",
            "Gunakan informasi yang tersedia tanpa menyimpulkan seluruh perangkat sehat.",
        ));
    }
    DiagnosticRun {
        started_at,
        finished_at: Utc::now(),
        days: days.clamp(1, 30),
        findings,
    }
}
#[cfg(windows)]
fn storage(findings: &mut Vec<Finding>) {
    use serde_json::Value;
    use std::collections::BTreeMap;
    let result = (|| -> anyhow::Result<Vec<BTreeMap<String, Value>>> {
        let c = wmi::WMIConnection::with_namespace_path("ROOT\\Microsoft\\Windows\\Storage")?;
        Ok(c.raw_query(
            "SELECT FriendlyName,HealthStatus,OperationalStatus FROM MSFT_PhysicalDisk",
        )?)
    })();
    match result {
        Ok(rows) if !rows.is_empty() => {
            for row in rows {
                let name = row
                    .get("FriendlyName")
                    .and_then(Value::as_str)
                    .unwrap_or("Storage");
                let health = row.get("HealthStatus").and_then(Value::as_u64);
                let (severity, status) = match health {
                    Some(0) => (Severity::Info, Status::NotDetected),
                    Some(1) => (Severity::Warning, Status::Detected),
                    Some(2) => (Severity::Critical, Status::Detected),
                    _ => (Severity::Info, Status::Unavailable),
                };
                findings.push(finding("Storage",severity,status,name,"Status yang dilaporkan storage provider Windows, bukan pemeriksaan menyeluruh semua atribut SMART.",&format!("MSFT_PhysicalDisk; HealthStatus {}; OperationalStatus {}",health.map(|v|v.to_string()).unwrap_or_else(||"Tidak tersedia".into()),row.get("OperationalStatus").map(ToString::to_string).unwrap_or_else(||"Tidak tersedia".into())),"Jika Windows melaporkan masalah, cadangkan data dan periksa perangkat."));
            }
        }
        Err(e) => findings.push(finding(
            "Storage",
            Severity::Info,
            Status::Unavailable,
            "Status storage tidak tersedia",
            "Izin/provider tidak mendukung pemeriksaan ini.",
            &e.to_string(),
            "Periksa dukungan storage Windows.",
        )),
        _ => findings.push(finding(
            "Storage",
            Severity::Info,
            Status::Unavailable,
            "Tidak ada hasil storage",
            "Provider tidak mengembalikan physical disk.",
            "MSFT_PhysicalDisk",
            "Periksa dukungan storage Windows.",
        )),
    }
}
#[cfg(not(windows))]
fn storage(findings: &mut Vec<Finding>) {
    findings.push(finding(
        "Storage",
        Severity::Info,
        Status::Unavailable,
        "Tidak tersedia",
        "Windows diperlukan",
        "Capability",
        "Gunakan Windows.",
    ));
}
#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn real_diagnostics_have_evidence() {
        let r = super::scan(1);
        assert!(!r.findings.is_empty());
        assert!(
            r.findings
                .iter()
                .all(|f| !f.evidence.is_empty() && !f.can_auto_fix)
        );
        assert_ne!(r.summary(), "Laptop 100% sehat");
    }
}
