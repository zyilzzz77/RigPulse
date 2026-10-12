use crate::{Application, cleanup::RemovedApp};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct History {
    pub removed: Vec<RemovedApp>,
    pub pending_restart: Vec<Application>,
}
impl History {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let path = dir.join("apps/history.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let _guards = crate::safe_fs::guard_path(&path, false)?;
        anyhow::ensure!(
            std::fs::metadata(&path)?.len() <= 4 * 1024 * 1024,
            "Riwayat aplikasi terlalu besar"
        );
        let mut history: Self = serde_json::from_slice(&std::fs::read(path)?)?;
        history.removed.truncate(256);
        history.pending_restart.truncate(32);
        Ok(history)
    }
    pub fn save(&self, dir: &Path) -> anyhow::Result<()> {
        write_json(&dir.join("apps/history.json"), self)
    }
    pub fn record_removed(&mut self, app: Application) {
        self.removed.retain(|r| r.app.id != app.id);
        self.removed.push(RemovedApp {
            app,
            verified_at: chrono::Utc::now(),
        });
        if self.removed.len() > 256 {
            self.removed.remove(0);
        }
    }
}
pub(crate) fn write_json(path: &Path, value: &(impl Serialize + ?Sized)) -> anyhow::Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Storage path tidak valid"))?;
    if !parent.exists() {
        let mut missing = Vec::new();
        let mut current = parent.to_path_buf();
        while !current.exists() {
            missing.push(current.clone());
            anyhow::ensure!(current.pop(), "Storage path tidak valid");
        }
        let _guards = crate::safe_fs::guard_path(&current, false)?;
        for p in missing.iter().rev() {
            std::fs::create_dir(p)?;
            crate::safe_fs::guard_path(p, false)?;
        }
    }
    let _guards = crate::safe_fs::guard_path(parent, false)?;
    if path.exists() {
        crate::safe_fs::guard_path(path, false)?;
    }
    let temp = parent.join(format!("{}.tmp", crate::cleanup::unique_id()));
    let bytes = serde_json::to_vec_pretty(value)?;
    anyhow::ensure!(
        bytes.len() <= 4 * 1024 * 1024,
        "Data lokal melebihi batas 4 MiB"
    );
    let result = (|| -> anyhow::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        let file = crate::safe_fs::open_file(&temp, true)?;
        crate::safe_fs::rename_replace(&file, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}
