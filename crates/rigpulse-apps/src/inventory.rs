use crate::Inventory;
use std::sync::atomic::{AtomicBool, Ordering};
pub fn scan(cancel: &AtomicBool, progress: impl Fn(usize)) -> anyhow::Result<Inventory> {
    anyhow::ensure!(!cancel.load(Ordering::Relaxed), "Pemindaian dibatalkan");
    #[cfg(windows)]
    {
        crate::native::scan(cancel, progress)
    }
    #[cfg(not(windows))]
    {
        let _ = progress;
        anyhow::bail!("Inventaris aplikasi membutuhkan Windows")
    }
}
