pub fn is_admin() -> bool {
    unsafe { windows::Win32::UI::Shell::IsUserAnAdmin().as_bool() }
}
pub(crate) struct Operation(windows::Win32::Foundation::HANDLE);
impl Operation {
    pub(crate) fn acquire() -> anyhow::Result<Self> {
        use windows::{
            Win32::{
                Foundation::{CloseHandle, WAIT_ABANDONED, WAIT_OBJECT_0},
                System::Threading::{CreateMutexW, WaitForSingleObject},
            },
            core::w,
        };
        unsafe {
            let handle = CreateMutexW(None, false, w!("Local\\RigPulseOfficialDriverOperation"))?;
            let wait = WaitForSingleObject(handle, 0);
            if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
                let _ = CloseHandle(handle);
                anyhow::bail!("Operasi driver RigPulse lain masih berjalan");
            }
            Ok(Self(handle))
        }
    }
}
impl Drop for Operation {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::System::Threading::ReleaseMutex(self.0);
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
/// Starts a restricted Drivers-only UI. No package IDs, scripts, or arbitrary commands cross UAC.
/// The elevated process searches WUA again and requires its own explicit selection and confirmation.
pub fn open_manager() -> anyhow::Result<()> {
    use windows::{
        Win32::{
            Foundation::{CloseHandle, WAIT_FAILED, WAIT_OBJECT_0},
            System::Threading::WaitForSingleObject,
            UI::{Shell::*, WindowsAndMessaging::SW_SHOWNORMAL},
        },
        core::{PCWSTR, w},
    };
    let exe: Vec<u16> = std::env::current_exe()?
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(exe.as_ptr()),
        lpParameters: w!("--driver-admin"),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe {
        ShellExecuteExW(&mut info)?;
        if !info.hProcess.is_invalid() {
            loop {
                let wait = WaitForSingleObject(info.hProcess, 500);
                if wait == WAIT_OBJECT_0 {
                    break;
                }
                if wait == WAIT_FAILED {
                    let _ = CloseHandle(info.hProcess);
                    anyhow::bail!("Gagal menunggu jendela administrator");
                }
            }
            let _ = CloseHandle(info.hProcess);
        }
    }
    Ok(())
}
pub fn restart_confirmed() -> anyhow::Result<()> {
    use std::os::windows::process::CommandExt;
    // Static absolute OS executable; no caller-provided command, host, or arguments.
    let root = std::env::var_os("SystemRoot")
        .ok_or_else(|| anyhow::anyhow!("SystemRoot tidak tersedia"))?;
    let output =
        std::process::Command::new(std::path::PathBuf::from(root).join("System32/shutdown.exe"))
            .args(["/r", "/t", "0"])
            .creation_flags(0x08000000)
            .output()?;
    anyhow::ensure!(
        output.status.success(),
        "Windows menolak restart: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
