//! Rust adapter for the official, separately licensed Ookla CLI. No shell or PATH lookup.
use crate::{Cancellation, check_cancel, tools::SpeedResult};
use sha2::{Digest, Sha256};
use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
const URL: &str = "https://install.speedtest.net/app/cli/ookla-speedtest-1.2.0-win64.zip";
const ZIP_HASH: &str = "13e3d888b845d301a556419e31f14ab9bff57e3f06089ef2fd3bdc9ba6841efa";
const EXE_HASH: &str = "c73c4ac2a7cdc48829fe0f9d7ec91ca41c8b434490d4eb3205aad69c7aa3b921";
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn word(bytes: &[u8], at: usize) -> anyhow::Result<u16> {
    Ok(u16::from_le_bytes(
        bytes
            .get(at..at + 2)
            .ok_or_else(|| anyhow::anyhow!("Arsip Ookla terpotong"))?
            .try_into()?,
    ))
}
fn dword(bytes: &[u8], at: usize) -> anyhow::Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(at..at + 4)
            .ok_or_else(|| anyhow::anyhow!("Arsip Ookla terpotong"))?
            .try_into()?,
    ))
}
// Only the checksum-pinned publisher archive is accepted; no arbitrary ZIP paths are extracted.
fn extract(bytes: &[u8], name: &str) -> anyhow::Result<Vec<u8>> {
    anyhow::ensure!(
        hash(bytes) == ZIP_HASH,
        "Checksum paket Ookla berbeda; unduhan ditolak"
    );
    let end = (bytes.len().saturating_sub(65_557)..bytes.len().saturating_sub(21))
        .rev()
        .find(|&n| bytes.get(n..n + 4) == Some(b"PK\x05\x06"))
        .ok_or_else(|| anyhow::anyhow!("Direktori arsip tidak ditemukan"))?;
    let count = word(bytes, end + 10)?;
    let mut pos = dword(bytes, end + 16)? as usize;
    for _ in 0..count {
        anyhow::ensure!(
            bytes.get(pos..pos + 4) == Some(b"PK\x01\x02"),
            "Direktori arsip tidak valid"
        );
        let length = word(bytes, pos + 28)? as usize;
        let extra = word(bytes, pos + 30)? as usize;
        let comment = word(bytes, pos + 32)? as usize;
        if bytes.get(pos + 46..pos + 46 + length) == Some(name.as_bytes()) {
            let compressed = dword(bytes, pos + 20)? as usize;
            let size = dword(bytes, pos + 24)? as usize;
            let local = dword(bytes, pos + 42)? as usize;
            anyhow::ensure!(
                size <= 4_194_304 && word(bytes, pos + 8)? & 1 == 0,
                "Arsip melebihi batas / terenkripsi"
            );
            anyhow::ensure!(
                bytes.get(local..local + 4) == Some(b"PK\x03\x04"),
                "Header arsip tidak valid"
            );
            let start =
                local + 30 + word(bytes, local + 26)? as usize + word(bytes, local + 28)? as usize;
            let payload = bytes
                .get(start..start + compressed)
                .ok_or_else(|| anyhow::anyhow!("Isi arsip terpotong"))?;
            let mut out = Vec::new();
            match word(bytes, pos + 10)? {
                0 => out.extend_from_slice(payload),
                8 => {
                    flate2::read::DeflateDecoder::new(payload)
                        .take(size as u64 + 1)
                        .read_to_end(&mut out)?;
                }
                _ => anyhow::bail!("Kompresi arsip tidak didukung"),
            }
            anyhow::ensure!(out.len() == size, "Ukuran isi arsip tidak valid");
            return Ok(out);
        }
        pos += 46 + length + extra + comment;
    }
    anyhow::bail!("{name} tidak ditemukan dalam paket resmi")
}
fn prepare(
    dir: &Path,
    cancel: &Cancellation,
    progress: &impl Fn(String),
) -> anyhow::Result<PathBuf> {
    let folder = dir.join("tools/ookla-1.2.0");
    let exe = folder.join("speedtest.exe");
    if exe.exists() {
        anyhow::ensure!(
            std::fs::metadata(&exe)?.len() <= 4_194_304,
            "Cache Ookla terlalu besar"
        );
        let bytes = std::fs::read(&exe)?;
        anyhow::ensure!(
            hash(&bytes) == EXE_HASH,
            "File Ookla berubah; hapus cache Ookla dan unduh kembali"
        );
        return Ok(exe);
    }
    progress("Mengunduh CLI resmi Ookla (sekali) dan memverifikasi SHA-256…".into());
    let client = reqwest::blocking::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(40))
        .build()?;
    let mut response = client.get(URL).send()?.error_for_status()?;
    let mut bytes = Vec::with_capacity(2_000_000);
    let mut buf = [0u8; 65_536];
    loop {
        check_cancel(cancel)?;
        let n = response.read(&mut buf)?;
        if n == 0 {
            break;
        }
        anyhow::ensure!(bytes.len() + n <= 4_194_304, "Paket Ookla terlalu besar");
        bytes.extend_from_slice(&buf[..n]);
    }
    let binary = extract(&bytes, "speedtest.exe")?;
    anyhow::ensure!(
        hash(&binary) == EXE_HASH,
        "Checksum executable Ookla tidak valid"
    );
    let documentation = extract(&bytes, "speedtest.md")?;
    check_cancel(cancel)?;
    std::fs::create_dir_all(&folder)?;
    std::fs::write(folder.join("speedtest.md"), documentation)?;
    std::fs::write(folder.join("speedtest.pending"), binary)?;
    std::fs::rename(folder.join("speedtest.pending"), &exe)?;
    Ok(exe)
}
fn nonnegative(value: &serde_json::Value) -> Option<f64> {
    value.as_f64().filter(|v| v.is_finite() && *v >= 0.0)
}
pub fn parse_result(v: &serde_json::Value, elapsed: f64) -> anyhow::Result<SpeedResult> {
    anyhow::ensure!(
        v["type"].as_str() == Some("result"),
        "Hasil akhir Ookla belum tersedia"
    );
    let bandwidth = |kind: &str| {
        nonnegative(&v[kind]["bandwidth"])
            .map(|n| n * 8.0 / 1_000_000.0)
            .ok_or_else(|| anyhow::anyhow!("Hasil {kind} Ookla tidak valid"))
    };
    Ok(SpeedResult {
        download_mbps: bandwidth("download")?,
        upload_mbps: bandwidth("upload")?,
        latency_ms: nonnegative(&v["ping"]["latency"]),
        jitter_ms: nonnegative(&v["ping"]["jitter"]),
        packet_loss: nonnegative(&v["packetLoss"]),
        server: format!(
            "{} · {} · ID {}",
            v["server"]["name"].as_str().unwrap_or("Tidak tersedia"),
            v["server"]["location"].as_str().unwrap_or(""),
            v["server"]["id"]
        ),
        bytes: v["download"]["bytes"]
            .as_u64()
            .zip(v["upload"]["bytes"].as_u64())
            .map(|(download, upload)| download.saturating_add(upload)),
        elapsed,
        at: v["timestamp"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&chrono::Utc))
            .unwrap_or_else(chrono::Utc::now),
    })
}
/// Typed live measurement events sourced from Ookla `--format=jsonl
/// --progress=yes` records. `DownloadSample`/`UploadSample` carry actual
/// reported bandwidth; no values are synthesized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeedStage {
    Preparing,
    Connecting,
    TestingPing,
    TestingDownload,
    TestingUpload,
    Finalizing,
}
impl SpeedStage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Preparing => "Menyiapkan pengujian...",
            Self::Connecting => "Menghubungi server tes...",
            Self::TestingPing => "Mengukur latensi...",
            Self::TestingDownload => "DOWNLOAD",
            Self::TestingUpload => "UPLOAD",
            Self::Finalizing => "Menghitung hasil akhir...",
        }
    }
}
#[derive(Clone, Debug)]
pub enum SpeedEvent {
    Stage(SpeedStage),
    DownloadSample { mbps: f64, progress: Option<f64> },
    UploadSample { mbps: f64, progress: Option<f64> },
    Message(String),
}
struct Child(std::process::Child);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
pub fn run(
    dir: &Path,
    cancel: &Cancellation,
    progress: impl Fn(String),
) -> anyhow::Result<SpeedResult> {
    run_with_events(dir, cancel, |event| match event {
        SpeedEvent::Stage(s) => progress(s.label().into()),
        SpeedEvent::DownloadSample { mbps, progress: p } => {
            let pct = p
                .map(|p| format!("{:.0}%", p.clamp(0.0, 1.0) * 100.0))
                .unwrap_or_default();
            progress(format!("Ookla Download {pct} · {mbps:.2} Mbps"));
        }
        SpeedEvent::UploadSample { mbps, progress: p } => {
            let pct = p
                .map(|p| format!("{:.0}%", p.clamp(0.0, 1.0) * 100.0))
                .unwrap_or_default();
            progress(format!("Ookla Upload {pct} · {mbps:.2} Mbps"));
        }
        SpeedEvent::Message(m) => progress(m),
    })
}
pub fn run_with_events(
    dir: &Path,
    cancel: &Cancellation,
    event: impl Fn(SpeedEvent),
) -> anyhow::Result<SpeedResult> {
    check_cancel(cancel)?;
    use std::os::windows::process::CommandExt;
    let progress_msg = |m: String| event(SpeedEvent::Message(m));
    let exe = prepare(dir, cancel, &progress_msg)?;
    check_cancel(cancel)?;
    event(SpeedEvent::Stage(SpeedStage::Connecting));
    event(SpeedEvent::Message(
        "Ookla: memilih server dan mengukur ping…".into(),
    ));
    let mut child = Child(
        Command::new(exe)
            .args([
                "--accept-license",
                "--accept-gdpr",
                "--format=jsonl",
                "--progress=yes",
                "--progress-update-interval=250",
            ])
            .creation_flags(0x08000000)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let stdout = child
        .0
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("Output Ookla tidak tersedia"))?;
    let stderr = child
        .0
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("Status Ookla tidak tersedia"))?;
    let (tx, rx) = std::sync::mpsc::sync_channel(32);
    std::thread::spawn(move || {
        for line in BufReader::new(stdout.take(1_048_577)).lines() {
            let Ok(line) = line else {
                break;
            };
            if line.len() > 65_536 {
                break;
            }
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let errors = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.take(32_768).read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    });
    let started = Instant::now();
    let mut result = None;
    let mut transferred = [0u64; 2];
    loop {
        check_cancel(cancel)?;
        anyhow::ensure!(
            started.elapsed() < Duration::from_secs(120),
            "Ookla melewati batas 120 detik; tes dihentikan"
        );
        for line in rx.try_iter() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                match v["type"].as_str() {
                    Some("result") => {
                        result = Some(parse_result(&v, started.elapsed().as_secs_f64())?)
                    }
                    Some(kind @ ("download" | "upload")) => {
                        if let Some(bytes) = v[kind]["bytes"].as_u64() {
                            let index = usize::from(kind == "upload");
                            transferred[index] = transferred[index].max(bytes);
                            anyhow::ensure!(
                                transferred[0].saturating_add(transferred[1]) <= 512 * 1024 * 1024,
                                "Tes Ookla dihentikan: trafik terlapor melewati batas 512 MiB. Tidak ada hasil akhir yang diklaim."
                            );
                        }
                        let p = nonnegative(&v[kind]["progress"]);
                        let rate = nonnegative(&v[kind]["bandwidth"]);
                        // Only genuine CLI-reported bandwidth drives the gauge.
                        if kind == "download" {
                            event(SpeedEvent::Stage(SpeedStage::TestingDownload));
                            if let Some(r) = rate {
                                event(SpeedEvent::DownloadSample {
                                    mbps: r * 8.0 / 1_000_000.0,
                                    progress: p,
                                });
                            }
                        } else {
                            event(SpeedEvent::Stage(SpeedStage::TestingUpload));
                            if let Some(r) = rate {
                                event(SpeedEvent::UploadSample {
                                    mbps: r * 8.0 / 1_000_000.0,
                                    progress: p,
                                });
                            }
                        }
                    }
                    Some("ping") => {
                        event(SpeedEvent::Stage(SpeedStage::TestingPing));
                        event(SpeedEvent::Message(
                            "Ookla: mengukur latensi dan jitter…".into(),
                        ));
                    }
                    _ => {}
                }
            }
        }
        if let Some(status) = child.0.try_wait()? {
            // Read records already queued after process termination before consuming the final result.
            for line in rx.iter() {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line)
                    && v["type"].as_str() == Some("result")
                {
                    result = Some(parse_result(&v, started.elapsed().as_secs_f64())?);
                }
            }
            let error = errors.join().unwrap_or_default();
            anyhow::ensure!(
                status.success(),
                "Ookla gagal: {}",
                error.chars().take(1000).collect::<String>()
            );
            return result
                .ok_or_else(|| anyhow::anyhow!("Ookla selesai tanpa hasil tes yang valid"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelled_before_start_performs_no_download_or_launch() {
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let dir =
            std::env::temp_dir().join(format!("rigpulse-cancel-no-io-{}", std::process::id()));
        assert!(run(&dir, &cancel, |_| {}).is_err());
        assert!(!dir.exists());
    }
    #[test]
    fn parses_ookla_bytes_per_second_without_fabricating_latency() {
        let v = serde_json::json!({"type":"result","download":{"bandwidth":12500000,"bytes":10},"upload":{"bandwidth":1250000,"bytes":20},"server":{"name":"Fixture only","id":1},"ping":{"latency":9.5}});
        let r = parse_result(&v, 2.0).unwrap();
        assert_eq!(r.download_mbps, 100.0);
        assert_eq!(r.upload_mbps, 10.0);
        assert_eq!(r.bytes, Some(30));
        let mut missing_bytes = v.clone();
        missing_bytes["upload"]
            .as_object_mut()
            .unwrap()
            .remove("bytes");
        assert_eq!(parse_result(&missing_bytes, 2.0).unwrap().bytes, None);
        assert_eq!(r.latency_ms, Some(9.5));
        assert_eq!(r.jitter_ms, None);
        assert!(parse_result(&serde_json::Value::Null, 1.0).is_err());
        assert!(extract(b"arbitrary", "speedtest.exe").is_err());
    }
    #[test]
    #[ignore = "Reads the separately downloaded official archive; no test traffic or license acceptance"]
    fn official_archive_hash_and_extraction() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../artifacts/ookla-speedtest-1.2.0-win64.zip");
        let zip = std::fs::read(path).unwrap();
        assert_eq!(hash(&extract(&zip, "speedtest.exe").unwrap()), EXE_HASH);
        assert!(extract(&zip, "speedtest.md").unwrap().len() > 1000);
    }
}
