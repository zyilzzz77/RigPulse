#[cfg(windows)]
pub(crate) struct ComApartment;
#[cfg(windows)]
impl ComApartment {
    pub(crate) fn new() -> anyhow::Result<Self> {
        unsafe {
            windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_MULTITHREADED,
            )
            .ok()?;
        }
        Ok(Self)
    }
}
#[cfg(windows)]
impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            windows::Win32::System::Com::CoUninitialize();
        }
    }
}

#[cfg(windows)]
pub fn search() -> anyhow::Result<rigpulse_core::UpdateSearch> {
    search_with(
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        |_| {},
    )
}
#[cfg(windows)]
pub fn search_with(
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    progress: impl Fn(crate::install::Progress),
) -> anyhow::Result<rigpulse_core::UpdateSearch> {
    use windows::{
        Win32::System::{Com::*, UpdateAgent::*},
        core::{BSTR, Interface},
    };
    let _com = ComApartment::new()?;
    unsafe {
        let session: IUpdateSession = CoCreateInstance(&UpdateSession, None, CLSCTX_INPROC_SERVER)?;
        session.SetClientApplicationID(&BSTR::from("RigPulse"))?;
        let searcher = session.CreateUpdateSearcher()?;
        // Leave server selection/service/policy at Windows defaults. No configuration changes.
        let result = crate::async_wua::search(
            &searcher,
            &BSTR::from("IsInstalled=0 and IsHidden=0 and Type='Driver'"),
            &cancel,
            &progress,
        )?;
        let code = result.ResultCode()?;
        anyhow::ensure!(
            code == orcSucceeded || code == orcSucceededWithErrors,
            "WUA search result {}",
            code.0
        );
        let collection = result.Updates()?;
        let mut updates = Vec::new();
        for n in 0..collection.Count()? {
            let update = collection.get_Item(n)?;
            let driver: IWindowsDriverUpdate = update.cast()?;
            let identity = update.Identity()?;
            let title = update.Title()?.to_string();
            let class = driver.DriverClass()?.to_string();
            let hardware_id = driver.DriverHardwareID()?.to_string();
            let eligible = rigpulse_core::class_allowed(&class, &hardware_id, &title);
            let license_accepted = update.EulaAccepted()?.as_bool();
            let description = update.Description().ok().map(|s| s.to_string());
            let max_download_bytes = update
                .MaxDownloadSize()
                .ok()
                .and_then(|d| windows::Win32::System::Ole::VarR8FromDec(&d).ok());
            let restart_possible = update
                .InstallationBehavior()
                .ok()
                .and_then(|b| b.RebootBehavior().ok())
                .map(|b| b != irbNeverReboots);
            updates.push(rigpulse_core::DriverUpdate {description,max_download_bytes,restart_possible,license_accepted,id:identity.UpdateID()?.to_string(),revision:identity.RevisionNumber()?,title,class,hardware_id,model:driver.DriverModel().map(|s|s.to_string()).unwrap_or_else(|_|"Tidak tersedia".into()),provider:driver.DriverProvider().map(|s|s.to_string()).unwrap_or_else(|_|"Windows Update".into()),downloaded:update.IsDownloaded()?.as_bool(),eligible:eligible&&license_accepted,reason:if !license_accepted {Some("Lisensi paket belum diterima. Tinjau dan setujui lisensi melalui Windows Update.".into())}else if eligible {None}else{Some("Firmware/BIOS, kelas tidak dikenal atau perangkat kritis dikecualikan dari instalasi otomatis.".into())}});
        }
        Ok(rigpulse_core::UpdateSearch {
            at: chrono::Utc::now(),
            source: "Windows Update Agent / kebijakan Windows setempat".into(),
            updates,
            warning: if code == orcSucceededWithErrors {
                Some(
                    "Pencarian selesai sebagian. Sebagian sumber WUA tidak dapat diperiksa.".into(),
                )
            } else {
                None
            },
        })
    }
}
#[cfg(not(windows))]
pub fn search() -> anyhow::Result<rigpulse_core::UpdateSearch> {
    anyhow::bail!("WUA hanya tersedia di Windows")
}
#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn com_apartment_can_initialize() {
        let handle = std::thread::spawn(|| super::ComApartment::new().map(|_| ()));
        assert!(handle.join().unwrap().is_ok());
    }
    #[test]
    #[ignore = "Online WUA probe; may take minutes and respects OS policy"]
    fn real_wua_read_only() {
        let result = std::thread::spawn(super::search).join().unwrap();
        match result {
            Ok(s) => assert!(s.updates.iter().all(|u| !u.id.is_empty())),
            Err(e) => panic!("WUA returned a real API error: {e}"),
        }
    }
}
