use rigpulse_core::UpdateSearch;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallState {
    AwaitingConfirmation,
    Downloading,
    Installing,
    Verifying,
    Completed,
    RestartRequired,
    PartiallyCompleted,
    Downloaded,
    Cancelled,
    Failed,
}
#[derive(Debug, Clone)]
pub struct Consent {
    pub(crate) selected: Vec<(String, i32)>,
    pub(crate) confirmed_at: chrono::DateTime<chrono::Utc>,
}
impl Consent {
    pub fn package_count(&self) -> usize {
        self.selected.len()
    }
    pub fn confirm(
        search: &UpdateSearch,
        ids: &[String],
        acknowledged: bool,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(acknowledged, "Persetujuan eksplisit diperlukan");
        anyhow::ensure!(
            search.source == "Windows Update Agent / kebijakan Windows setempat"
                && search.warning.is_none(),
            "Sumber/pencarian tidak tervalidasi"
        );
        anyhow::ensure!(!ids.is_empty(), "Tidak ada paket yang dipilih");
        anyhow::ensure!(
            (chrono::Utc::now() - search.at).num_minutes() < 30,
            "Pencarian kedaluwarsa; periksa pembaruan lagi"
        );
        let mut selected = Vec::new();
        for id in ids {
            let u = search
                .updates
                .iter()
                .find(|u| &u.id == id)
                .ok_or_else(|| anyhow::anyhow!("Paket tidak ada dalam penawaran Windows"))?;
            anyhow::ensure!(
                u.eligible && rigpulse_core::class_allowed(&u.class, &u.hardware_id, &u.title),
                "Paket tidak diizinkan"
            );
            anyhow::ensure!(
                !selected.iter().any(|(prior, _)| prior == id),
                "Paket duplikat"
            );
            selected.push((id.clone(), u.revision));
        }
        Ok(Self {
            selected,
            confirmed_at: chrono::Utc::now(),
        })
    }
    pub fn valid(&self) -> bool {
        !self.selected.is_empty() && (chrono::Utc::now() - self.confirmed_at).num_minutes() < 10
    }
}
pub struct StateMachine {
    state: InstallState,
    consent: Option<Consent>,
}
impl Default for StateMachine {
    fn default() -> Self {
        Self {
            state: InstallState::AwaitingConfirmation,
            consent: None,
        }
    }
}
impl StateMachine {
    pub fn approve(&mut self, consent: Consent) {
        self.consent = Some(consent);
    }
    pub fn transition(&mut self, next: InstallState) -> anyhow::Result<()> {
        use InstallState::*;
        let allowed = matches!(
            (self.state, next),
            (AwaitingConfirmation, Downloading)
                | (Downloading, Installing)
                | (Installing, Verifying)
                | (
                    Verifying,
                    Completed | RestartRequired | PartiallyCompleted | Cancelled
                )
                | (Downloading, Downloaded | Cancelled | PartiallyCompleted)
                | (
                    AwaitingConfirmation | Downloading | Installing | Verifying,
                    Failed
                )
        );
        anyhow::ensure!(allowed, "Transisi instalasi tidak valid");
        if matches!(next, Downloading | Installing) {
            anyhow::ensure!(
                self.consent.as_ref().is_some_and(Consent::valid),
                "Persetujuan tidak ada atau kedaluwarsa"
            );
        }
        self.state = next;
        Ok(())
    }
    pub fn state(&self) -> InstallState {
        self.state
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageResult {
    pub update_id: String,
    pub result_code: i32,
    pub hresult: i32,
    pub restart_required: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallationResult {
    pub state: InstallState,
    pub packages: Vec<PackageResult>,
    pub verification_note: String,
}

#[derive(Default)]
pub struct DownloadEta {
    previous: Option<(f64, f64)>,
    rate: Option<f64>,
    last_progress: Option<f64>,
}
impl DownloadEta {
    pub fn update(&mut self, at: f64, bytes: f64, total: f64) -> Option<f64> {
        if !at.is_finite() || !bytes.is_finite() || bytes < 0.0 {
            self.rate = None;
            self.previous = None;
            return None;
        }
        let prior = if let Some(prior) = self.previous {
            prior
        } else {
            self.previous = Some((at, bytes));
            return None;
        };
        if bytes < prior.1 || at <= prior.0 {
            self.rate = None;
            self.previous = Some((at, bytes));
            self.last_progress = None;
            return None;
        }
        if at - prior.0 >= 1.0 {
            let rate = (bytes - prior.1) / (at - prior.0);
            self.previous = Some((at, bytes));
            if bytes > prior.1 {
                self.last_progress = Some(at);
            }
            if rate > 0.0 || self.rate.is_some() {
                self.rate = Some(self.rate.map(|old| old * 0.7 + rate * 0.3).unwrap_or(rate));
            }
        }
        if self.last_progress.is_none_or(|last| at - last > 5.0) {
            return None;
        }
        self.rate
            .filter(|r| *r > 0.0 && total > bytes && total.is_finite())
            .map(|r| (total - bytes) / r)
    }
}
#[derive(Debug, Clone)]
pub struct Progress {
    pub state: InstallState,
    pub percent: Option<i32>,
    pub downloaded: Option<f64>,
    pub total: Option<f64>,
    pub eta_seconds: Option<f64>,
    pub elapsed: f64,
    pub abort_requested: bool,
}
impl Progress {
    pub(crate) fn state(state: InstallState) -> Self {
        Self {
            state,
            percent: None,
            downloaded: None,
            total: None,
            eta_seconds: None,
            elapsed: 0.0,
            abort_requested: false,
        }
    }
}

/// Every operation still requires fresh, explicitly confirmed WUA package identities.
pub const PRODUCTION_INSTALL_ENABLED: bool = true;

#[cfg(windows)]
pub fn execute(
    consent: Consent,
    download_only: bool,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    progress: impl Fn(Progress),
) -> anyhow::Result<InstallationResult> {
    use windows::{
        Win32::Foundation::VARIANT_BOOL,
        Win32::System::{Com::*, UpdateAgent::*},
        core::{BSTR, Interface},
    };
    anyhow::ensure!(consent.valid(), "Persetujuan kedaluwarsa");
    let _operation = crate::privilege::Operation::acquire()?;
    let _com = super::wua::ComApartment::new()?;
    let mut machine = StateMachine::default();
    machine.approve(consent.clone());
    anyhow::ensure!(
        download_only || crate::privilege::is_admin(),
        "Izin administrator diperlukan; buka pengelola driver dengan izin admin"
    );
    anyhow::ensure!(
        !cancel.load(std::sync::atomic::Ordering::Relaxed),
        "Dibatalkan"
    );
    unsafe {
        let session: IUpdateSession = CoCreateInstance(&UpdateSession, None, CLSCTX_INPROC_SERVER)?;
        session.SetClientApplicationID(&BSTR::from("RigPulse official driver management"))?;
        let installer = session.CreateUpdateInstaller()?;
        anyhow::ensure!(
            !installer.IsBusy()?.as_bool(),
            "Windows sedang memasang pembaruan lain; tunggu hingga selesai"
        );
        let system: ISystemInformation =
            CoCreateInstance(&SystemInformation, None, CLSCTX_INPROC_SERVER)?;
        anyhow::ensure!(
            download_only || !system.RebootRequired()?.as_bool(),
            "Windows memerlukan restart sebelum pemasangan berikutnya"
        );
        let offered = crate::async_wua::search(
            &session.CreateUpdateSearcher()?,
            &BSTR::from("IsInstalled=0 and IsHidden=0 and Type='Driver'"),
            &cancel,
            &progress,
        )?;
        anyhow::ensure!(
            offered.ResultCode()? == orcSucceeded,
            "Pencarian ulang tidak lengkap"
        );
        let all = offered.Updates()?;
        let chosen: IUpdateCollection =
            CoCreateInstance(&UpdateCollection, None, CLSCTX_INPROC_SERVER)?;
        let mut hardware_ids = Vec::new();
        for (id, revision) in &consent.selected {
            let mut found = false;
            for n in 0..all.Count()? {
                let update = all.get_Item(n)?;
                let identity = update.Identity()?;
                if identity.UpdateID()? != id.as_str() || identity.RevisionNumber()? != *revision {
                    continue;
                }
                let d: IWindowsDriverUpdate = update.cast()?;
                let class = d.DriverClass()?.to_string();
                let hw = d.DriverHardwareID()?.to_string();
                anyhow::ensure!(
                    rigpulse_core::class_allowed(&class, &hw, &update.Title()?.to_string()),
                    "Paket ditolak saat validasi ulang"
                );
                anyhow::ensure!(
                    update.EulaAccepted()?.as_bool(),
                    "Lisensi belum disetujui. Gunakan Windows Update untuk meninjau lisensi"
                );
                chosen.Add(&update)?;
                hardware_ids.push(hw);
                found = true;
                break;
            }
            anyhow::ensure!(found, "Penawaran Windows berubah; periksa ulang");
        }
        anyhow::ensure!(
            consent.valid(),
            "Persetujuan kedaluwarsa selama validasi ulang; konfirmasikan lagi"
        );
        progress(Progress::state(InstallState::Downloading));
        machine.transition(InstallState::Downloading)?;
        let downloader = session.CreateUpdateDownloader()?;
        downloader.SetUpdates(&chosen)?;
        downloader.SetIsForced(VARIANT_BOOL::from(false))?;
        let download = crate::async_wua::download(&downloader, &cancel, &progress)?;
        let mut downloaded_packages = Vec::new();
        for n in 0..chosen.Count()? {
            let r = download.GetUpdateResult(n)?;
            downloaded_packages.push(PackageResult {
                update_id: chosen.get_Item(n)?.Identity()?.UpdateID()?.to_string(),
                result_code: r.ResultCode()?.0,
                hresult: r.HResult()?,
                restart_required: false,
            });
        }
        if download_only
            || download.ResultCode()? != orcSucceeded
            || cancel.load(std::sync::atomic::Ordering::Relaxed)
        {
            let state = if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                InstallState::Cancelled
            } else if download.ResultCode()? == orcSucceeded {
                InstallState::Downloaded
            } else {
                InstallState::PartiallyCompleted
            };
            return Ok(InstallationResult {
                state: {
                    machine.transition(state)?;
                    machine.state()
                },
                packages: downloaded_packages,
                verification_note:
                    "Hasil unduhan resmi WUA. Driver belum dipasang oleh operasi ini.".into(),
            });
        }
        progress(Progress::state(InstallState::Installing));
        machine.transition(InstallState::Installing)?;
        installer.SetUpdates(&chosen)?;
        installer.SetIsForced(VARIANT_BOOL::from(false))?;
        installer.SetAllowSourcePrompts(VARIANT_BOOL::from(false))?;
        anyhow::ensure!(
            !installer.RebootRequiredBeforeInstallation()?.as_bool(),
            "Windows memerlukan restart sebelum instalasi"
        );
        anyhow::ensure!(
            !installer.IsBusy()?.as_bool(),
            "Windows sedang memasang pembaruan lain"
        );
        let result = crate::async_wua::install(&installer, &cancel, &progress)?;
        let mut packages = Vec::new();
        for n in 0..chosen.Count()? {
            let r = result.GetUpdateResult(n)?;
            packages.push(PackageResult {
                update_id: chosen.get_Item(n)?.Identity()?.UpdateID()?.to_string(),
                result_code: r.ResultCode()?.0,
                hresult: r.HResult()?,
                restart_required: r.RebootRequired()?.as_bool(),
            });
        }
        progress(Progress::state(InstallState::Verifying));
        machine.transition(InstallState::Verifying)?;
        let devices = rigpulse_windows::devices::enumerate();
        let verified = devices.as_ref().is_ok_and(|devices| {
            hardware_ids.iter().all(|id| {
                devices.iter().any(|d| {
                    d.problem == Some(0)
                        && d.hardware_ids.iter().any(|h| h.eq_ignore_ascii_case(id))
                })
            })
        });
        let code = result.ResultCode()?;
        let state =
            if result.RebootRequired()?.as_bool() || packages.iter().any(|p| p.restart_required) {
                InstallState::RestartRequired
            } else if code == orcAborted {
                InstallState::Cancelled
            } else if code != orcSucceeded || !verified {
                InstallState::PartiallyCompleted
            } else {
                InstallState::Completed
            };
        progress(Progress::state(state));
        machine.transition(state)?;
        Ok(InstallationResult{state,packages,verification_note:if verified{"Hardware ID yang ditawarkan ditemukan kembali tanpa problem code. Verifikasi versi setelah restart tetap diperlukan."}else{"Hasil WUA diterima tetapi status perangkat belum terverifikasi lengkap; periksa Device Manager setelah restart."}.into()})
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eta_needs_measured_progress_and_resets_on_regression() {
        let mut eta = DownloadEta::default();
        assert_eq!(eta.update(0.0, 0.0, 100.0), None);
        assert_eq!(eta.update(2.0, 0.0, 100.0), None);
        assert_eq!(eta.update(4.0, 20.0, 100.0), Some(8.0));
        assert_eq!(eta.update(6.0, 5.0, 100.0), None);
        assert_eq!(eta.update(8.0, 25.0, 100.0), Some(7.5));
        assert_eq!(eta.update(16.0, 25.0, 100.0), None);
    }
    fn search() -> UpdateSearch {
        UpdateSearch {
            at: chrono::Utc::now(),
            source: "Windows Update Agent / kebijakan Windows setempat".into(),
            warning: None,
            updates: vec![rigpulse_core::DriverUpdate {
                description: None,
                max_download_bytes: None,
                restart_possible: Some(false),
                license_accepted: true,
                id: "test-package".into(),
                revision: 1,
                title: "Display".into(),
                class: "Display".into(),
                model: "test".into(),
                provider: "test".into(),
                hardware_id: "PCI\\TEST".into(),
                downloaded: false,
                eligible: true,
                reason: None,
            }],
        }
    }
    #[test]
    fn cannot_download_without_consent() {
        let mut s = StateMachine::default();
        assert!(s.transition(InstallState::Downloading).is_err());
        assert!(s.transition(InstallState::Installing).is_err());
    }
    #[test]
    fn explicit_consent_and_order() {
        let source = search();
        let ids = vec!["test-package".into()];
        assert!(Consent::confirm(&source, &ids, false).is_err());
        let mut s = StateMachine::default();
        s.approve(Consent::confirm(&source, &ids, true).unwrap());
        s.transition(InstallState::Downloading).unwrap();
        assert!(s.transition(InstallState::Completed).is_err());
        s.transition(InstallState::Installing).unwrap();
        s.transition(InstallState::Verifying).unwrap();
        s.transition(InstallState::RestartRequired).unwrap();
        assert!(s.transition(InstallState::Failed).is_err());
    }
    #[test]
    fn rejects_untrusted_source_firmware_and_stale() {
        let mut source = search();
        let ids = vec!["test-package".into()];
        source.source = "untrusted".into();
        assert!(Consent::confirm(&source, &ids, true).is_err());
        source = search();
        source.updates[0].class = "Firmware".into();
        assert!(Consent::confirm(&source, &ids, true).is_err());
        source = search();
        source.at -= chrono::Duration::hours(1);
        assert!(Consent::confirm(&source, &ids, true).is_err());
    }
}
