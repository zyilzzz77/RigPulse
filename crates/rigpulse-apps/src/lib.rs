//! Native installed software, official uninstall and conservative recoverable cleanup.
mod model;
pub use model::*;
pub mod cleanup;
pub mod history;
pub mod icons;
pub mod inventory;
#[cfg(windows)]
mod native;
pub mod query;
mod safe_fs;
pub mod uninstall;
pub fn open_install_folder(path: &std::path::Path) -> anyhow::Result<()> {
    native::validate_local_path(path)?;
    let _guards = safe_fs::guard_path(path, false)?;
    anyhow::ensure!(path.is_dir(), "Folder tidak tersedia");
    let target = native::wide(path);
    let result = unsafe {
        windows::Win32::UI::Shell::ShellExecuteW(
            None,
            windows::core::w!("explore"),
            windows::core::PCWSTR(target.as_ptr()),
            None,
            None,
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        )
    };
    anyhow::ensure!(result.0 as isize > 32, "Windows tidak dapat membuka folder");
    Ok(())
}
