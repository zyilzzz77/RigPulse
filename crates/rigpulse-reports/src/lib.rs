use rigpulse_core::{BatteryReading, DeviceDriverInfo, DiagnosticRun, Inventory, UpdateSearch};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub app_version: String,
    pub exported_at: chrono::DateTime<chrono::Utc>,
    pub sensitive_included: bool,
    pub inventory: Option<Inventory>,
    pub battery: Option<BatteryReading>,
    pub devices: Option<Vec<DeviceDriverInfo>>,
    pub updates: Option<UpdateSearch>,
    pub monitoring: Option<rigpulse_monitor::Sample>,
    pub diagnostics: Option<DiagnosticRun>,
}
impl Report {
    pub fn private(mut self, include: bool) -> Self {
        self.sensitive_included = include;
        self.inventory = self.inventory.map(|i| i.private(include));
        self.devices = self
            .devices
            .map(|d| d.into_iter().map(|d| d.private(include)).collect());
        if let Some(s) = &mut self.updates {
            s.updates = s.updates.iter().map(|u| u.private(include)).collect();
        }
        self
    }
    pub fn json(&self, path: &Path) -> anyhow::Result<()> {
        write_atomic(
            path,
            &serde_json::to_vec_pretty(&self.clone().private(self.sensitive_included))?,
        )
    }
    pub fn pdf(&self, path: &Path) -> anyhow::Result<()> {
        self.clone()
            .private(self.sensitive_included)
            .render_pdf(path)
    }
    fn render_pdf(&self, path: &Path) -> anyhow::Result<()> {
        use genpdf::{Element, elements, fonts, style};
        let font = |name: &str| -> anyhow::Result<fonts::FontData> {
            Ok(fonts::FontData::new(
                std::fs::read(Path::new("C:/Windows/Fonts").join(name))?,
                None,
            )?)
        };
        let family = fonts::FontFamily {
            regular: font("arial.ttf")?,
            bold: font("arialbd.ttf")?,
            italic: font("ariali.ttf")?,
            bold_italic: font("arialbi.ttf")?,
        };
        let mut doc = genpdf::Document::new(family);
        doc.set_title("RigPulse - Laporan perangkat");
        doc.set_font_size(10);
        doc.set_line_spacing(1.25);
        let mut decorator = genpdf::SimplePageDecorator::new();
        decorator.set_margins(18);
        decorator.set_header(|page| {let mut h=elements::LinearLayout::vertical();h.push(elements::Paragraph::new(format!("RIGPULSE  /  LAPORAN PERANGKAT                                       HALAMAN {page}")).styled(style::Style::new().with_font_size(8).with_color(style::Color::Rgb(90,105,125))));h.push(elements::Break::new(1));h});
        doc.set_page_decorator(decorator);
        let heading = |text: String| {
            elements::Paragraph::new(text).styled(
                style::Style::new()
                    .bold()
                    .with_font_size(16)
                    .with_color(style::Color::Rgb(16, 27, 46)),
            )
        };
        doc.push(heading("Laporan perangkat".into()));
        doc.push(elements::Paragraph::new(format!(
            "RigPulse {} | Ekspor UTC: {}",
            self.app_version,
            self.exported_at.format("%Y-%m-%d %H:%M:%S")
        )));
        doc.push(elements::Paragraph::new(if self.sensitive_included {
            "Identitas sensitif disertakan atas pilihan pengguna."
        } else {
            "Privasi: serial number, PnP ID, hardware ID dan identitas jaringan disembunyikan."
        }));
        doc.push(elements::Break::new(1));
        if let Some(run) = &self.diagnostics {
            doc.push(heading(run.summary().into()));
            doc.push(elements::Paragraph::new(format!("Rentang Event Log: {} hari. Status dibatasi pemeriksaan yang berhasil. Sensor yang tidak didukung bukan bukti perangkat sehat.",run.days)));
            for f in &run.findings {
                doc.push(elements::Break::new(0.5));
                doc.push(
                    elements::Paragraph::new(format!(
                        "[{:?} / {:?}] {} - {}",
                        f.severity, f.status, f.category, f.title
                    ))
                    .styled(style::Style::new().bold()),
                );
                doc.push(elements::Paragraph::new(&f.explanation));
                doc.push(
                    elements::Paragraph::new(format!("Evidence: {}", f.evidence))
                        .styled(style::Style::new().with_font_size(9)),
                );
                doc.push(elements::Paragraph::new(format!(
                    "Saran: {}",
                    f.recommended_action
                )));
            }
        }
        if let Some(i) = &self.inventory {
            for s in &i.sections {
                doc.push(elements::Break::new(1));
                doc.push(heading(s.name.clone()));
                doc.push(
                    elements::Paragraph::new(format!(
                        "Sumber: {} | UTC {}",
                        s.source,
                        i.at.format("%Y-%m-%d %H:%M:%S")
                    ))
                    .styled(style::Style::new().with_font_size(9)),
                );
                if let Some(e) = &s.error {
                    doc.push(elements::Paragraph::new(format!("Tidak tersedia: {e}")));
                }
                for p in &s.properties {
                    doc.push(elements::Paragraph::new(format!("{}: {}", p.name, p.value)));
                }
            }
        }
        if let Some(b) = &self.battery {
            doc.push(elements::Break::new(1));
            doc.push(heading("Baterai".into()));
            doc.push(elements::Paragraph::new(format!(
                "Windows Battery API | Baterai: {} | Daya tersisa: {} | Adaptor AC: {}",
                bool_text(b.present),
                option_text(b.percent, "%"),
                bool_text(b.ac)
            )));
            for (n, p) in b.packs.iter().enumerate() {
                doc.push(elements::Paragraph::new(format!(
                    "Baterai {}: kapasitas awal {}; penuh saat ini {}; kesehatan {}; siklus {}",
                    n + 1,
                    p.design_mwh
                        .map(|v| format!("{:.2} Wh", f64::from(v) / 1000.0))
                        .unwrap_or_else(|| "Tidak tersedia".into()),
                    p.full_mwh
                        .map(|v| format!("{:.2} Wh", f64::from(v) / 1000.0))
                        .unwrap_or_else(|| "Tidak tersedia".into()),
                    p.health()
                        .map(|v| format!("{v:.1}%"))
                        .unwrap_or_else(|| "Tidak tersedia".into()),
                    option_text(p.cycles, "")
                )));
            }
        }
        if let Some(s) = &self.monitoring {
            doc.push(elements::Break::new(1));
            doc.push(heading("Snapshot monitoring".into()));
            for m in [&s.cpu, &s.memory].into_iter().chain(s.extra.iter()) {
                doc.push(elements::Paragraph::new(format!(
                    "{}: {} | {} | UTC {}",
                    m.label,
                    m.text(),
                    m.source,
                    m.at.format("%H:%M:%S")
                )));
            }
        }
        if let Some(devices) = &self.devices {
            doc.push(elements::Break::new(1));
            doc.push(heading("Driver terpasang".into()));
            doc.push(elements::Paragraph::new("Sumber: SetupAPI / Configuration Manager. Status perangkat bukan status pembaruan."));
            for d in devices {
                doc.push(elements::Paragraph::new(format!(
                    "{} | {} | {} | {} (kode Windows {})",
                    d.name,
                    d.class,
                    d.version.as_deref().unwrap_or("Tidak tersedia"),
                    match d.status {
                        rigpulse_core::DeviceStatus::Working => "Berfungsi",
                        rigpulse_core::DeviceStatus::Disabled => "Dinonaktifkan",
                        rigpulse_core::DeviceStatus::MissingDriver => "Driver belum terpasang",
                        rigpulse_core::DeviceStatus::Issue => "Perlu perhatian",
                        rigpulse_core::DeviceStatus::Unknown => "Belum diketahui",
                    },
                    option_text(d.problem, "")
                )));
                if self.sensitive_included {
                    doc.push(elements::Paragraph::new(format!("PnP: {}", d.instance_id)));
                }
            }
        }
        if let Some(search) = &self.updates {
            doc.push(elements::Break::new(1));
            doc.push(heading("Penawaran pembaruan".into()));
            doc.push(elements::Paragraph::new(format!(
                "{} | UTC {}",
                search.source, search.at
            )));
            for u in &search.updates {
                doc.push(elements::Paragraph::new(format!(
                    "{} | {} | {}",
                    u.title, u.provider, u.class
                )));
            }
            if search.updates.is_empty() {
                doc.push(elements::Paragraph::new(
                    "Tidak ada driver yang ditawarkan saat pencarian.",
                ));
            }
        }
        let mut bytes = Vec::new();
        doc.render(&mut bytes)?;
        write_atomic(path, &repair_font_unicode(&bytes)?)
    }
}
// printpdf 0.3 emits scalar hex for non-BMP glyphs. PDF ToUnicode destinations
// require UTF-16BE instead. Repair only font CMap streams, then rebuild xrefs.
fn repair_font_unicode(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut doc = lopdf::Document::load_mem(bytes)?;
    for object in doc.objects.values_mut() {
        if let lopdf::Object::Stream(stream) = object {
            let content = if stream.dict.has(b"Filter") {
                stream.decompressed_content()?
            } else {
                stream.content.clone()
            };
            if let Ok(text) = std::str::from_utf8(&content)
                && text.contains("begincmap")
            {
                let repaired = repair_cmap(text);
                stream.dict.remove(b"Filter");
                stream.dict.remove(b"DecodeParms");
                stream.set_content(repaired.into_bytes());
            }
        }
    }
    let mut output = Vec::new();
    doc.save_to(&mut output)?;
    Ok(output)
}
fn repair_cmap(text: &str) -> String {
    let mut result = String::new();
    for line in text.lines() {
        let parts: Vec<_> = line.split_whitespace().collect();
        if parts.len() == 2
            && parts[0].starts_with('<')
            && parts[0].ends_with('>')
            && parts[1].starts_with('<')
            && parts[1].ends_with('>')
        {
            let hex = &parts[1][1..parts[1].len() - 1];
            if let Ok(raw_code) = u32::from_str_radix(hex, 16)
                && (hex.len() > 4 || matches!(raw_code, 0xa0 | 0xad | 0x37e))
                && let Some(c) = char::from_u32(match raw_code {
                    0xa0 => 0x20,
                    0xad => 0x2d,
                    0x37e => 0x3b,
                    c => c,
                })
            {
                use std::fmt::Write;
                let mut buffer = [0u16; 2];
                let encoded = c.encode_utf16(&mut buffer);
                result.push_str(parts[0]);
                result.push_str(" <");
                for unit in encoded {
                    let _ = write!(result, "{unit:04x}");
                }
                result.push_str(">\n");
                continue;
            }
        }
        result.push_str(line);
        result.push('\n');
    }
    result
}
fn bool_text(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "Ya",
        Some(false) => "Tidak",
        None => "Tidak tersedia",
    }
}
fn option_text(value: Option<impl std::fmt::Display>, unit: &str) -> String {
    value
        .map(|v| format!("{v}{unit}"))
        .unwrap_or_else(|| "Tidak tersedia".into())
}
fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)?;
    use std::io::Write;
    let result = (|| -> anyhow::Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn font_map_uses_utf16_surrogate_pairs() {
        let output =
            repair_cmap("1 beginbfchar\n<0042> <1f600>\nendbfchar\n<0043> <00b0>\n<0003> <00a0>\n");
        assert!(output.contains("<0042> <d83dde00>"));
        assert!(output.contains("<0043> <00b0>"));
        assert!(output.contains("<0003> <0020>"));
    }
    #[test]
    fn privacy_redacts_device_and_update_identifiers() {
        let report = Report {
            app_version: "test".into(),
            exported_at: chrono::Utc::now(),
            sensitive_included: true,
            inventory: None,
            battery: None,
            monitoring: None,
            diagnostics: None,
            updates: None,
            devices: Some(vec![DeviceDriverInfo {
                name: "Test device".into(),
                class: "Display".into(),
                manufacturer: None,
                provider: None,
                version: None,
                date: None,
                status: rigpulse_core::DeviceStatus::Unknown,
                problem: None,
                instance_id: "SECRET_INSTANCE".into(),
                hardware_ids: vec!["SECRET_HW".into()],
            }]),
        }
        .private(false);
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("SECRET"));
        assert!(!report.sensitive_included);
    }
}
