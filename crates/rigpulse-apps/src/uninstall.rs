use crate::{Application, Inventory};
use anyhow::ensure;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    Desktop {
        executable: PathBuf,
        parameters: String,
    },
    Msi {
        product: String,
    },
    Store {
        full_name: String,
    },
}
pub fn valid_guid(s: &str) -> bool {
    s.len() == 38
        && s.starts_with('{')
        && s.ends_with('}')
        && s.as_bytes()[1..37].iter().enumerate().all(|(n, b)| {
            if [8, 13, 18, 23].contains(&n) {
                *b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
pub fn plan(app: &Application) -> anyhow::Result<Plan> {
    ensure!(
        app.removable(),
        "Aplikasi dilindungi atau metode uninstall tidak tersedia"
    );
    if app.kind == crate::AppKind::Msi {
        let product = app
            .product_code
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Identitas MSI tidak tersedia"))?;
        ensure!(valid_guid(product), "Identitas MSI tidak valid");
        return Ok(Plan::Msi {
            product: product.clone(),
        });
    }
    let command = app
        .uninstall
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Uninstaller tidak terdaftar"))?;
    if app.kind == crate::AppKind::Store {
        ensure!(
            !command.is_empty()
                && command.len() < 512
                && !command.contains(['\\', '/', '\0', '"'])
                && command.contains('_'),
            "Identitas paket tidak valid"
        );
        return Ok(Plan::Store {
            full_name: command.into(),
        });
    }
    parse_command(command)
}
/// Parse the executable boundary only; preserve the vendor's exact arguments without a shell.
pub fn parse_command(command: &str) -> anyhow::Result<Plan> {
    ensure!(
        command.len() < 32760 && !command.contains(['\0', '\r', '\n']),
        "Perintah uninstall tidak valid"
    );
    let s = command.trim();
    let (exe, tail) = if let Some(quoted) = s.strip_prefix('"') {
        let end = quoted
            .find('"')
            .ok_or_else(|| anyhow::anyhow!("Kutip tidak lengkap"))?;
        ensure!(
            quoted[end + 1..].is_empty() || quoted[end + 1..].starts_with(char::is_whitespace),
            "Batas executable tidak valid"
        );
        (&quoted[..end], quoted[end + 1..].trim())
    } else {
        let end = s.find(char::is_whitespace).unwrap_or(s.len());
        (&s[..end], s[end..].trim())
    };
    let executable = PathBuf::from(exe);
    ensure!(
        executable
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("exe")),
        "Hanya uninstaller EXE langsung yang didukung; gunakan Settings Windows untuk metode lain"
    );
    let filename = executable
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    ensure!(
        ![
            "cmd.exe",
            "powershell.exe",
            "pwsh.exe",
            "rundll32.exe",
            "regsvr32.exe",
            "wscript.exe",
            "cscript.exe",
            "mshta.exe",
            "msiexec.exe",
            "explorer.exe",
            "reg.exe",
            "schtasks.exe"
        ]
        .contains(&filename.as_str()),
        "Peluncur shell/script ditolak"
    );
    #[cfg(windows)]
    {
        crate::native::validate_local_path(&executable)?;
        crate::safe_fs::guard_path(&executable, false)?;
    }
    ensure!(
        executable.is_file(),
        "Executable uninstall tidak tersedia atau jalurnya tidak aman"
    );
    Ok(Plan::Desktop {
        executable,
        parameters: tail.into(),
    })
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Removed,
    RestartRequired,
    Cancelled,
    Unverified,
}
#[derive(Debug)]
pub struct UninstallResult {
    pub outcome: Outcome,
    pub inventory: Option<Inventory>,
    pub exit_code: Option<u32>,
}
pub fn execute(
    app: &Application,
    approved: bool,
    cancel: &AtomicBool,
    progress: impl Fn(&str),
) -> anyhow::Result<UninstallResult> {
    execute_with_elevation(app, approved, false, cancel, progress)
}
/// Elevation is a separate UI-approved attempt; only the official uninstaller receives UAC.
pub fn execute_with_elevation(
    app: &Application,
    approved: bool,
    elevate: bool,
    cancel: &AtomicBool,
    progress: impl Fn(&str),
) -> anyhow::Result<UninstallResult> {
    ensure!(approved, "Persetujuan uninstall diperlukan");
    ensure!(
        !cancel.load(Ordering::Relaxed),
        "Dibatalkan sebelum uninstall"
    );
    progress("Menyiapkan uninstaller…");
    let before = crate::inventory::scan(cancel, |_| {})?;
    let current = before
        .apps
        .iter()
        .find(|a| a.id == app.id)
        .ok_or_else(|| anyhow::anyhow!("Aplikasi berubah; segarkan daftar"))?;
    ensure!(
        current == app,
        "Metadata aplikasi berubah; konfirmasi ulang setelah menyegarkan daftar"
    );
    let prepared = plan(current)?;
    #[cfg(windows)]
    let code = execute_native(prepared, elevate, cancel, &progress)?;
    #[cfg(not(windows))]
    let code: Option<u32> = {
        let _ = prepared;
        anyhow::bail!("Uninstall membutuhkan Windows")
    };
    if cancel.load(Ordering::Relaxed) || code == Some(1602) {
        return Ok(UninstallResult {
            outcome: Outcome::Cancelled,
            inventory: None,
            exit_code: code,
        });
    }
    let restart = matches!(code, Some(3010 | 1641));
    ensure!(
        code.is_none() || matches!(code, Some(0 | 3010 | 1641)),
        "Uninstaller selesai dengan kode {}",
        code.unwrap_or_default()
    );
    progress("Memverifikasi penghapusan aplikasi…");
    let mut after = None;
    for attempt in 0..4 {
        if cancel.load(Ordering::Relaxed) {
            return Ok(UninstallResult {
                outcome: Outcome::Cancelled,
                inventory: after,
                exit_code: code,
            });
        }
        let inventory = crate::inventory::scan(cancel, |_| {})?;
        if removal_verified(app, &inventory) {
            return Ok(UninstallResult {
                outcome: if restart {
                    Outcome::RestartRequired
                } else {
                    Outcome::Removed
                },
                inventory: Some(inventory),
                exit_code: code,
            });
        }
        after = Some(inventory);
        if attempt < 3 {
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }
    Ok(UninstallResult {
        outcome: if restart {
            Outcome::RestartRequired
        } else {
            Outcome::Unverified
        },
        inventory: after,
        exit_code: code,
    })
}
pub fn removal_verified(app: &Application, inventory: &Inventory) -> bool {
    inventory.completed_sources.contains(&app.source)
        && !inventory.apps.iter().any(|a| app.same_family(a))
}
#[cfg(windows)]
fn execute_native(
    plan: Plan,
    elevate: bool,
    cancel: &AtomicBool,
    progress: &impl Fn(&str),
) -> anyhow::Result<Option<u32>> {
    use windows::{
        Win32::{
            Foundation::*,
            System::Threading::*,
            UI::{Shell::*, WindowsAndMessaging::SW_SHOWNORMAL},
        },
        core::{HSTRING, PCWSTR},
    };
    if let Plan::Store { full_name } = plan {
        ensure!(
            !elevate,
            "Paket Store hanya dihapus untuk pengguna saat ini; elevasi tidak didukung"
        );
        let _apartment = crate::native::Apartment::new()?;
        progress("Menghapus paket pengguna saat ini melalui Windows…");
        let manager = windows::Management::Deployment::PackageManager::new()?;
        let operation = manager.RemovePackageAsync(&HSTRING::from(full_name))?;
        while operation.Status()?.0 == 0 {
            if cancel.load(Ordering::Relaxed) {
                operation.Cancel()?;
                return Ok(Some(1602));
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        let result = operation.GetResults()?;
        result.ExtendedErrorCode()?.ok()?;
        return Ok(Some(0));
    }
    let (executable, parameters) = match plan {
        Plan::Msi { product } => {
            let system = crate::native::known_folder(&FOLDERID_System)?;
            (
                system.join("msiexec.exe"),
                format!("/x {product} /norestart"),
            )
        }
        Plan::Desktop {
            executable,
            parameters,
        } => (executable, parameters),
        _ => unreachable!(),
    };
    let exe = crate::native::wide(&executable);
    let guards = crate::safe_fs::guard_path(&executable, false)?;
    let args = crate::native::wide(&parameters);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: if elevate {
            windows::core::w!("runas")
        } else {
            windows::core::w!("open")
        },
        lpFile: PCWSTR(exe.as_ptr()),
        lpParameters: PCWSTR(args.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    progress("Menjalankan uninstaller resmi…");
    unsafe {
        ShellExecuteExW(&mut info)?;
    }
    drop(guards);
    if info.hProcess.is_invalid() {
        return Ok(None);
    }
    struct Process(HANDLE);
    impl Drop for Process {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    let process = Process(info.hProcess);
    progress("Menunggu uninstaller. Selesaikan dialog aplikasi yang terbuka…");
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(Some(1602));
        }
        let wait = unsafe { WaitForSingleObject(process.0, 250) };
        if wait == WAIT_OBJECT_0 {
            let mut code = 0;
            unsafe {
                GetExitCodeProcess(process.0, &mut code)?;
            }
            return Ok(Some(code));
        }
        ensure!(wait == WAIT_TIMEOUT, "Gagal memantau uninstaller");
    }
}
