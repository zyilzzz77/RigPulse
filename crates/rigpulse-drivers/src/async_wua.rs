use crate::install::{DownloadEta, InstallState, Progress};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use windows::{
    Win32::System::{Ole::VarR8FromDec, UpdateAgent::*, Variant::VARIANT},
    core::{BSTR, IUnknown, Interface, Ref, Result, implement},
};
#[implement(
    IDownloadCompletedCallback,
    IDownloadProgressChangedCallback,
    IInstallationCompletedCallback,
    IInstallationProgressChangedCallback,
    ISearchCompletedCallback
)]
struct Callbacks;
impl IDownloadCompletedCallback_Impl for Callbacks_Impl {
    fn Invoke(&self, _: Ref<IDownloadJob>, _: Ref<IDownloadCompletedCallbackArgs>) -> Result<()> {
        Ok(())
    }
}
impl IDownloadProgressChangedCallback_Impl for Callbacks_Impl {
    fn Invoke(
        &self,
        _: Ref<IDownloadJob>,
        _: Ref<IDownloadProgressChangedCallbackArgs>,
    ) -> Result<()> {
        Ok(())
    }
}
impl IInstallationCompletedCallback_Impl for Callbacks_Impl {
    fn Invoke(
        &self,
        _: Ref<IInstallationJob>,
        _: Ref<IInstallationCompletedCallbackArgs>,
    ) -> Result<()> {
        Ok(())
    }
}
impl IInstallationProgressChangedCallback_Impl for Callbacks_Impl {
    fn Invoke(
        &self,
        _: Ref<IInstallationJob>,
        _: Ref<IInstallationProgressChangedCallbackArgs>,
    ) -> Result<()> {
        Ok(())
    }
}
impl ISearchCompletedCallback_Impl for Callbacks_Impl {
    fn Invoke(&self, _: Ref<ISearchJob>, _: Ref<ISearchCompletedCallbackArgs>) -> Result<()> {
        Ok(())
    }
}
pub type Cancel = Arc<AtomicBool>;
macro_rules! job_guard {
    ($name:ident,$interface:ty) => {
        struct $name($interface);
        impl std::ops::Deref for $name {
            type Target = $interface;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
        impl Drop for $name {
            fn drop(&mut self) {
                unsafe {
                    if !self.0.IsCompleted().is_ok_and(|v| v.as_bool()) {
                        let _ = self.0.RequestAbort();
                    }
                    let _ = self.0.CleanUp();
                }
            }
        }
    };
}
job_guard!(SearchJob, ISearchJob);
job_guard!(DownloadJob, IDownloadJob);
job_guard!(InstallJob, IInstallationJob);
pub fn search(
    searcher: &IUpdateSearcher,
    criteria: &BSTR,
    cancel: &Cancel,
    progress: &impl Fn(Progress),
) -> anyhow::Result<ISearchResult> {
    unsafe {
        let cb: ISearchCompletedCallback = Callbacks.into();
        let unknown: IUnknown = cb.cast()?;
        let job = SearchJob(searcher.BeginSearch(criteria, &unknown, &VARIANT::default())?);
        let start = Instant::now();
        let mut aborted = false;
        while !job.IsCompleted()?.as_bool() {
            if !aborted && (cancel.load(Ordering::Relaxed) || start.elapsed().as_secs() > 900) {
                job.RequestAbort()?;
                aborted = true;
            }
            let mut p = Progress::state(InstallState::AwaitingConfirmation);
            p.elapsed = start.elapsed().as_secs_f64();
            p.abort_requested = aborted;
            progress(p);
            std::thread::sleep(Duration::from_millis(250));
        }
        let result = searcher.EndSearch(&job.0);
        anyhow::ensure!(!aborted, "Pencarian WUA dibatalkan");
        Ok(result?)
    }
}
pub fn download(
    downloader: &IUpdateDownloader,
    cancel: &Cancel,
    progress: &impl Fn(Progress),
) -> anyhow::Result<IDownloadResult> {
    unsafe {
        let cb: IDownloadCompletedCallback = Callbacks.into();
        let unknown: IUnknown = cb.cast()?;
        let job = DownloadJob(downloader.BeginDownload(&unknown, &unknown, &VARIANT::default())?);
        let start = Instant::now();
        let mut aborted = false;
        let mut eta = DownloadEta::default();
        while !job.IsCompleted()?.as_bool() {
            if !aborted && cancel.load(Ordering::Relaxed) {
                job.RequestAbort()?;
                aborted = true;
            }
            let mut p = Progress::state(InstallState::Downloading);
            p.elapsed = start.elapsed().as_secs_f64();
            p.abort_requested = aborted;
            if let Ok(current) = job.GetProgress() {
                p.percent = current
                    .PercentComplete()
                    .ok()
                    .filter(|v| (0..=100).contains(v));
                p.downloaded = current
                    .TotalBytesDownloaded()
                    .ok()
                    .and_then(|d| VarR8FromDec(&d).ok());
                p.total = current
                    .TotalBytesToDownload()
                    .ok()
                    .and_then(|d| VarR8FromDec(&d).ok());
                if let (Some(bytes), Some(total)) = (p.downloaded, p.total) {
                    p.eta_seconds = eta.update(p.elapsed, bytes, total);
                }
            }
            progress(p);
            std::thread::sleep(Duration::from_millis(250));
        }
        let result = downloader.EndDownload(&job.0);
        Ok(result?)
    }
}
pub fn install(
    installer: &IUpdateInstaller,
    cancel: &Cancel,
    progress: &impl Fn(Progress),
) -> anyhow::Result<IInstallationResult> {
    unsafe {
        let cb: IInstallationCompletedCallback = Callbacks.into();
        let unknown: IUnknown = cb.cast()?;
        let job = InstallJob(installer.BeginInstall(&unknown, &unknown, &VARIANT::default())?);
        let start = Instant::now();
        let mut aborted = false;
        while !job.IsCompleted()?.as_bool() {
            if !aborted && cancel.load(Ordering::Relaxed) {
                job.RequestAbort()?;
                aborted = true;
            }
            let mut p = Progress::state(InstallState::Installing);
            p.elapsed = start.elapsed().as_secs_f64();
            p.abort_requested = aborted;
            p.percent = job
                .GetProgress()
                .ok()
                .and_then(|p| p.PercentComplete().ok())
                .filter(|v| (0..=100).contains(v));
            progress(p);
            std::thread::sleep(Duration::from_millis(250));
        }
        let result = installer.EndInstall(&job.0);
        Ok(result?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_exposes_all_wua_async_interfaces() {
        let callback: IDownloadCompletedCallback = Callbacks.into();
        assert!(callback.cast::<IDownloadProgressChangedCallback>().is_ok());
        assert!(callback.cast::<IInstallationCompletedCallback>().is_ok());
        assert!(
            callback
                .cast::<IInstallationProgressChangedCallback>()
                .is_ok()
        );
        assert!(callback.cast::<ISearchCompletedCallback>().is_ok());
    }
}
