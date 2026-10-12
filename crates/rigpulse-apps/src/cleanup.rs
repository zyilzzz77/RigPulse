use crate::{Application, Inventory};
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovedApp {
    pub app: Application,
    pub verified_at: chrono::DateTime<chrono::Utc>,
}
#[derive(Debug, Clone)]
pub struct ScanRoots {
    pub roaming: PathBuf,
    pub local: PathBuf,
    pub data: PathBuf,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub id: String,
    pub app: Application,
    pub path: PathBuf,
    pub category: String,
    pub bytes: Option<u64>,
    pub confidence: String,
    pub risk: String,
    pub reason: String,
    pub eligible: bool,
    #[serde(default)]
    snapshot: Option<crate::safe_fs::Snapshot>,
}
#[derive(Debug, Clone, Default)]
pub struct ScanResult {
    pub candidates: Vec<Candidate>,
    pub warnings: Vec<String>,
}
pub struct Quarantine;
impl Quarantine {
    #[allow(
        clippy::too_many_arguments,
        reason = "Explicit approval, dry-run and cancellation at the destructive boundary"
    )]
    pub fn clean(
        roots: &ScanRoots,
        current: &Inventory,
        candidates: &[Candidate],
        selected: &[String],
        approved: bool,
        dry_run: bool,
        cancel: &AtomicBool,
        progress: impl Fn(usize),
    ) -> anyhow::Result<CleanupResult> {
        ensure!(approved, "Konfirmasi pembersihan diperlukan");
        ensure!(
            !selected.is_empty() && selected.len() <= 2500,
            "Pilihan kosong atau terlalu banyak"
        );
        let mut result = CleanupResult::default();
        let mut seen = std::collections::HashSet::new();
        for id in selected {
            ensure!(
                seen.insert(id) && candidates.iter().any(|c| &c.id == id && c.eligible),
                "Pilihan tidak valid atau tidak memenuhi syarat"
            );
        }
        if !dry_run {
            prepare_storage(roots)?;
        }
        let mut entries = if dry_run {
            Vec::new()
        } else {
            Self::entries(roots)?
        };
        let stored: u64 = entries.iter().map(|e| e.snapshot.bytes).sum();
        let incoming: u64 = candidates
            .iter()
            .filter(|c| selected.contains(&c.id))
            .map(|c| c.bytes.unwrap_or(0))
            .sum();
        ensure!(
            dry_run
                || (entries.len() + selected.len() <= 2500
                    && stored.saturating_add(incoming) <= 2 * 1024 * 1024 * 1024),
            "Karantina penuh (2 GiB / 2.500 item). Pulihkan atau hapus item karantina terlebih dahulu"
        );
        for (n, id) in selected.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                result.cancelled = true;
                break;
            }
            let c = candidates
                .iter()
                .find(|c| &c.id == id)
                .expect("validated selection");
            let moved = (|| -> anyhow::Result<u64> {
                ensure!(
                    crate::uninstall::removal_verified(&c.app, current),
                    "Aplikasi masih terpasang atau sumber tidak terbaca"
                );
                ensure!(
                    cache_root(&c.app, roots, &c.path).is_some() && !sensitive_file(&c.path),
                    "Jalur tidak termasuk cache yang diizinkan"
                );
                let _source_guards = crate::safe_fs::guard_path(
                    c.path
                        .parent()
                        .ok_or_else(|| anyhow::anyhow!("Jalur tidak valid"))?,
                    false,
                )?;
                let mut file = crate::safe_fs::open_file(&c.path, !dry_run)
                    .context("Membuka file terpilih")?;
                let actual = crate::safe_fs::snapshot(&mut file)?;
                ensure!(
                    c.snapshot.as_ref() == Some(&actual),
                    "Target berubah sejak scan; lakukan scan ulang"
                );
                if dry_run {
                    return Ok(0);
                }
                let storage = roots.data.join("apps/quarantine");
                let _destination_guards = crate::safe_fs::guard_path(&storage, false)?;
                let entry = QuarantineEntry {
                    id: unique_id(),
                    app: c.app.clone(),
                    original: c.path.clone(),
                    at: chrono::Utc::now(),
                    snapshot: actual.clone(),
                };
                let target = storage.join(format!("{}.bin", entry.id));
                let mut parent = crate::safe_fs::open_directory(&storage)?;
                ensure!(
                    crate::safe_fs::volume(&mut parent)? == actual.volume,
                    "Karantina lintas volume tidak didukung; file tidak diubah"
                );
                entries.push(entry.clone());
                save_entries(roots, &entries).context("Menyimpan jurnal pemulihan")?;
                if let Err(e) = crate::safe_fs::rename(&file, &target) {
                    entries.retain(|x| x.id != entry.id);
                    save_entries(roots, &entries)?;
                    return Err(e.context("Memindahkan file ke karantina"));
                }
                Ok(actual.bytes)
            })();
            match moved {
                Ok(_) if dry_run => result.skipped += 1,
                Ok(bytes) => {
                    result.moved += 1;
                    result.bytes += bytes;
                }
                Err(e) => result.failed.push(format!("{}: {e:#}", c.path.display())),
            }
            progress(n + 1);
        }
        Ok(result)
    }
    pub fn entries(roots: &ScanRoots) -> anyhow::Result<Vec<QuarantineEntry>> {
        let path = roots.data.join("apps/quarantine.json");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let _guards = crate::safe_fs::guard_path(&path, false)?;
        ensure!(
            std::fs::metadata(&path)?.len() <= 4 * 1024 * 1024,
            "Manifest karantina terlalu besar"
        );
        let entries: Vec<QuarantineEntry> = serde_json::from_slice(&std::fs::read(path)?)?;
        ensure!(entries.len() <= 2500, "Terlalu banyak entri karantina");
        for e in &entries {
            ensure!(
                !e.id.is_empty()
                    && e.id.len() < 96
                    && e.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
                "ID karantina tidak valid"
            );
        }
        Ok(entries)
    }
    pub fn recover(roots: &ScanRoots, id: &str, approved: bool, purge: bool) -> anyhow::Result<()> {
        ensure!(
            approved,
            "Konfirmasi pemulihan/penghapusan permanen diperlukan"
        );
        let mut entries = Self::entries(roots)?;
        let entry = entries
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| anyhow::anyhow!("Item tidak ditemukan dalam manifest"))?
            .clone();
        let source = roots
            .data
            .join("apps/quarantine")
            .join(format!("{}.bin", entry.id));
        let _q_guards = crate::safe_fs::guard_path(source.parent().unwrap(), false)?;
        let mut file = crate::safe_fs::open_file(&source, true)?;
        ensure!(
            crate::safe_fs::snapshot(&mut file)? == entry.snapshot,
            "File karantina berubah; pemulihan/penghapusan ditolak"
        );
        if purge {
            crate::safe_fs::delete(&file)?;
        } else {
            ensure!(
                cache_root(&entry.app, roots, &entry.original).is_some()
                    && !sensitive_file(&entry.original),
                "Jalur pemulihan tidak diizinkan"
            );
            ensure!(
                !entry.original.exists(),
                "Jalur asli sudah berisi file; pemulihan tidak akan menimpanya"
            );
            let _guards = crate::safe_fs::guard_path(entry.original.parent().unwrap(), false)?;
            crate::safe_fs::rename(&file, &entry.original)?;
        }
        drop(file);
        entries.retain(|e| e.id != id);
        save_entries(roots, &entries)?;
        Ok(())
    }
}
#[derive(Debug, Default)]
pub struct CleanupResult {
    pub moved: usize,
    pub bytes: u64,
    pub failed: Vec<String>,
    pub skipped: usize,
    pub cancelled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineEntry {
    pub id: String,
    pub app: Application,
    pub original: PathBuf,
    pub at: chrono::DateTime<chrono::Utc>,
    snapshot: crate::safe_fs::Snapshot,
}
impl QuarantineEntry {
    pub fn bytes(&self) -> u64 {
        self.snapshot.bytes
    }
}
pub(crate) fn unique_id() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}
fn prepare_storage(roots: &ScanRoots) -> anyhow::Result<()> {
    let mut missing = Vec::new();
    let mut path = roots.data.join("apps/quarantine");
    while !path.exists() {
        missing.push(path.clone());
        ensure!(path.pop(), "Storage root tidak tersedia");
    }
    let _guards = crate::safe_fs::guard_path(&path, false)?;
    for path in missing.iter().rev() {
        std::fs::create_dir(path)?;
        crate::safe_fs::guard_path(path, false)?;
    }
    Ok(())
}
fn save_entries(roots: &ScanRoots, entries: &[QuarantineEntry]) -> anyhow::Result<()> {
    crate::history::write_json(&roots.data.join("apps/quarantine.json"), entries)
}
impl ScanRoots {
    pub fn current(data: PathBuf) -> anyhow::Result<Self> {
        use windows::Win32::UI::Shell::{FOLDERID_LocalAppData, FOLDERID_RoamingAppData};
        Ok(Self {
            roaming: crate::native::known_folder(&FOLDERID_RoamingAppData)?,
            local: crate::native::known_folder(&FOLDERID_LocalAppData)?,
            data,
        })
    }
}
/// Deliberately narrow, reviewed identities. Unknown apps receive report-only installation candidates.
fn rule(app: &Application) -> Option<(&'static str, &'static str)> {
    let publisher = app.publisher.as_deref()?;
    if [
        "Microsoft Visual Studio Code",
        "Microsoft Visual Studio Code (User)",
        "Visual Studio Code",
    ]
    .contains(&app.name.as_str())
        && publisher == "Microsoft Corporation"
    {
        Some(("Code", "Software\\Microsoft\\VSCode"))
    } else if app.name == "Discord" && ["Discord Inc.", "Discord Inc"].contains(&publisher) {
        Some(("discord", "Software\\Discord"))
    } else if app.name == "Slack"
        && ["Slack Technologies, Inc.", "Slack Technologies LLC"].contains(&publisher)
    {
        Some(("Slack", "Software\\Slack"))
    } else {
        None
    }
}
fn within(child: &Path, parent: &Path) -> bool {
    let c = child
        .components()
        .map(|s| s.as_os_str().to_string_lossy().to_lowercase())
        .collect::<Vec<_>>();
    let p = parent
        .components()
        .map(|s| s.as_os_str().to_string_lossy().to_lowercase())
        .collect::<Vec<_>>();
    c.len() > p.len() && c.starts_with(&p)
}
fn cache_root(app: &Application, roots: &ScanRoots, path: &Path) -> Option<PathBuf> {
    if app.protected
        || app.name.eq_ignore_ascii_case("RigPulse")
        || within(path, &roots.data)
        || std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .is_some_and(|p| within(path, &p))
    {
        return None;
    }
    let (folder, _) = rule(app)?;
    for cache in ["Cache", "Code Cache", "GPUCache"] {
        let root = roots.roaming.join(folder).join(cache);
        if within(path, &root) {
            return Some(root);
        }
    }
    None
}
fn sensitive_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    [
        "password",
        "token",
        "credential",
        "login",
        "cookie",
        "history",
        "savegame",
    ]
    .iter()
    .any(|s| name.contains(s))
        || path.extension().is_some_and(|e| {
            [
                "json", "db", "sqlite", "sqlite3", "doc", "docx", "pdf", "xls", "xlsx", "jpg",
                "png", "zip", "rs", "py", "txt", "md", "pem", "key", "kdbx", "bak", "sql", "exe",
                "dll", "sys", "msi", "bat", "cmd", "ps1", "lnk", "js", "html", "cpp", "go",
            ]
            .contains(&e.to_string_lossy().to_lowercase().as_str())
        })
}
pub fn scan(
    history: &[RemovedApp],
    current: &Inventory,
    roots: &ScanRoots,
    registry: bool,
    cancel: &AtomicBool,
    progress: impl Fn(usize),
) -> anyhow::Result<ScanResult> {
    let mut output = ScanResult::default();
    let mut seen = std::collections::HashSet::new();
    let mut measured = 0u64;
    for removed in history.iter().rev().take(256) {
        ensure!(!cancel.load(Ordering::Relaxed), "Pemindaian dibatalkan");
        let app = &removed.app;
        if !crate::uninstall::removal_verified(app, current)
            || app.protected
            || app.name.eq_ignore_ascii_case("RigPulse")
        {
            continue;
        }
        if let Some((folder, registry_key)) = rule(app) {
            let base = roots.roaming.join(folder);
            if base.is_dir() {
                output.candidates.push(Candidate{id:format!("review:{}",app.id),app:app.clone(),path:base.clone(),category:"Konfigurasi / data pengguna".into(),bytes:None,
                    confidence:"Sedang".into(),risk:"Dilindungi — tinjau manual".into(),reason:"Lokasi data khusus aplikasi; konfigurasi dan data pribadi tidak dibersihkan.".into(),eligible:false,snapshot:None});
                for cache in ["Cache", "Code Cache", "GPUCache"] {
                    let root = base.join(cache);
                    if !root.exists() {
                        continue;
                    }
                    if let Err(e) = crate::safe_fs::guard_path(&root, false) {
                        output
                            .warnings
                            .push(format!("{} dilewati: {e}", root.display()));
                        continue;
                    }
                    let mut queue = vec![(root, 0usize)];
                    while let Some((dir, depth)) = queue.pop() {
                        ensure!(!cancel.load(Ordering::Relaxed), "Pemindaian dibatalkan");
                        if depth > 8
                            || output.candidates.len() >= 2500
                            || measured >= 512 * 1024 * 1024
                        {
                            output.warnings.push(
                                "Batas scan tercapai (2.500 item / kedalaman 8 / 512 MiB).".into(),
                            );
                            break;
                        }
                        let _guards = match crate::safe_fs::guard_path(&dir, false) {
                            Ok(g) => g,
                            Err(e) => {
                                output.warnings.push(e.to_string());
                                continue;
                            }
                        };
                        let entries = match std::fs::read_dir(&dir) {
                            Ok(e) => e,
                            Err(e) => {
                                output.warnings.push(e.to_string());
                                continue;
                            }
                        };
                        for entry in entries.take(2500) {
                            ensure!(!cancel.load(Ordering::Relaxed), "Pemindaian dibatalkan");
                            if output.candidates.len() >= 2500 || measured >= 512 * 1024 * 1024 {
                                break;
                            }
                            let entry = match entry {
                                Ok(e) => e,
                                Err(e) => {
                                    output.warnings.push(e.to_string());
                                    continue;
                                }
                            };
                            let path = entry.path();
                            if !seen.insert(path.clone()) {
                                continue;
                            }
                            if crate::safe_fs::guard_path(&path, false).is_err() {
                                output.warnings.push(format!(
                                    "Jalur tidak aman/tidak terbaca dilewati: {}",
                                    path.display()
                                ));
                                continue;
                            }
                            if entry.file_type()?.is_dir() {
                                queue.push((path, depth + 1));
                                continue;
                            }
                            let snapshot = if sensitive_file(&path) {
                                None
                            } else {
                                crate::safe_fs::open_file(&path, false)
                                    .and_then(|mut f| crate::safe_fs::snapshot(&mut f))
                                    .ok()
                            };
                            let eligible =
                                snapshot.is_some() && cache_root(app, roots, &path).is_some();
                            let bytes = snapshot.as_ref().map(|s| s.bytes);
                            measured += bytes.unwrap_or(0);
                            output.candidates.push(Candidate{id:format!("file:{}",path.display()),app:app.clone(),path,category:"Cache aplikasi".into(),bytes,
                                confidence:if eligible{"Tinggi"}else{"Rendah"}.into(),risk:if eligible{"Karantina dapat dipulihkan"}else{"Dilindungi / tidak dapat divalidasi"}.into(),
                                reason:if eligible{"Identitas/publisher dikenali, uninstall terverifikasi, keluarga aplikasi tidak terpasang, jalur cache khusus dan fingerprint file divalidasi."}else{"File sensitif, sedang digunakan, terlalu besar, atau tidak dapat dibaca dengan aman."}.into(),eligible,snapshot});
                            progress(output.candidates.len());
                        }
                    }
                }
            }
            if registry
                && crate::native::Key::open(
                    windows::Win32::System::Registry::HKEY_CURRENT_USER,
                    registry_key,
                    windows::Win32::System::Registry::KEY_WOW64_64KEY,
                )
                .is_ok()
            {
                output.candidates.push(Candidate{id:format!("registry:{}",app.id),app:app.clone(),path:PathBuf::from(format!("HKCU\\{registry_key}")),category:"Registry Leftovers — Advanced".into(),bytes:None,confidence:"Sedang".into(),risk:"Tinggi — laporan saja".into(),reason:"Key khusus aplikasi ditemukan; kepemilikan nilai/backup pemulihan belum dapat dijamin. Penghapusan dinonaktifkan.".into(),eligible:false,snapshot:None});
            }
        }
        if let Some(location) = &app.location
            && location.is_dir()
            && seen.insert(location.clone())
        {
            output.candidates.push(Candidate{id:format!("install:{}",app.id),app:app.clone(),path:location.clone(),category:"Sisa folder instalasi".into(),bytes:None,confidence:"Sedang".into(),risk:"Tinjau manual — mungkin dipakai bersama".into(),reason:"Jalur instalasi dilaporkan registry; tidak membuktikan seluruh folder aman dihapus.".into(),eligible:false,snapshot:None});
        }
    }
    Ok(output)
}
