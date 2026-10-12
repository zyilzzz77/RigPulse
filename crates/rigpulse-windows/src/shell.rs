#[cfg(windows)]
fn open(target: &std::ffi::OsStr) -> anyhow::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let path: Vec<u16> = target.encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        windows::Win32::UI::Shell::ShellExecuteW(
            None,
            windows::core::w!("open"),
            windows::core::PCWSTR(path.as_ptr()),
            None,
            None,
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        )
    };
    anyhow::ensure!(
        result.0 as isize > 32,
        "Windows tidak dapat membuka tujuan (kode {})",
        result.0 as isize
    );
    Ok(())
}
pub fn windows_update() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        open(std::ffi::OsStr::new("ms-settings:windowsupdate"))
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("Windows only")
    }
}
pub fn folder(path: &std::path::Path) -> anyhow::Result<()> {
    anyhow::ensure!(path.is_dir(), "Folder belum tersedia");
    #[cfg(windows)]
    {
        open(path.as_os_str())
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("Windows only")
    }
}
