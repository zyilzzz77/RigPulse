use crate::{AppKind, Application, Inventory};
use anyhow::{Context, ensure};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use windows::{
    Win32::{
        Foundation::*,
        System::{Registry::*, WinRT::*},
    },
    core::{HSTRING, PCWSTR, PWSTR},
};
pub(crate) fn wide(text: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    text.as_ref().encode_wide().chain(Some(0)).collect()
}
pub(crate) struct Apartment;
impl Apartment {
    pub(crate) fn new() -> anyhow::Result<Self> {
        unsafe { RoInitialize(RO_INIT_MULTITHREADED)? };
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { RoUninitialize() }
    }
}
pub(crate) struct Key(pub HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}
impl Key {
    pub(crate) fn open(root: HKEY, path: &str, view: REG_SAM_FLAGS) -> anyhow::Result<Self> {
        let mut key = HKEY::default();
        unsafe {
            RegOpenKeyExW(
                root,
                PCWSTR(wide(path).as_ptr()),
                None,
                KEY_READ | view,
                &mut key,
            )
            .ok()?
        };
        Ok(Self(key))
    }
    fn value(&self, name: &str) -> Option<(REG_VALUE_TYPE, Vec<u8>)> {
        let name = wide(name);
        let mut kind = REG_VALUE_TYPE::default();
        let mut len = 0;
        unsafe {
            RegQueryValueExW(
                self.0,
                PCWSTR(name.as_ptr()),
                None,
                Some(&mut kind),
                None,
                Some(&mut len),
            )
            .ok()
            .ok()?;
        }
        if len > 64 * 1024 {
            return None;
        }
        let mut bytes = vec![0; len as usize];
        unsafe {
            RegQueryValueExW(
                self.0,
                PCWSTR(name.as_ptr()),
                None,
                Some(&mut kind),
                Some(bytes.as_mut_ptr()),
                Some(&mut len),
            )
            .ok()
            .ok()?;
        }
        bytes.truncate(len as usize);
        Some((kind, bytes))
    }
    pub(crate) fn text(&self, name: &str) -> Option<String> {
        let (kind, bytes) = self.value(name)?;
        if kind != REG_SZ && kind != REG_EXPAND_SZ {
            return None;
        }
        let data: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .take_while(|&v| v != 0)
            .collect();
        let text = String::from_utf16(&data).ok()?.trim().to_string();
        if text.is_empty() || text.contains('\0') {
            None
        } else {
            Some(text)
        }
    }
    pub(crate) fn number(&self, name: &str) -> Option<u32> {
        let (kind, b) = self.value(name)?;
        if kind == REG_DWORD && b.len() == 4 {
            Some(u32::from_le_bytes(b.try_into().ok()?))
        } else {
            None
        }
    }
    fn children(&self) -> anyhow::Result<Vec<String>> {
        let mut result = Vec::new();
        for n in 0..10000 {
            let mut buf = [0u16; 512];
            let mut len = buf.len() as u32;
            let status = unsafe {
                RegEnumKeyExW(
                    self.0,
                    n,
                    Some(PWSTR(buf.as_mut_ptr())),
                    &mut len,
                    None,
                    None,
                    None,
                    None,
                )
            };
            if status == ERROR_NO_MORE_ITEMS {
                return Ok(result);
            }
            status.ok()?;
            result.push(String::from_utf16(&buf[..len as usize])?);
        }
        anyhow::bail!("Batas 10.000 entri registry tercapai")
    }
}
pub(crate) const UNINSTALL: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall";
pub fn scan(cancel: &AtomicBool, progress: impl Fn(usize)) -> anyhow::Result<Inventory> {
    let mut inventory = Inventory {
        apps: Vec::new(),
        scanned_at: chrono::Utc::now(),
        warnings: Vec::new(),
        completed_sources: Vec::new(),
    };
    for (root, scope) in [(HKEY_LOCAL_MACHINE, "HKLM"), (HKEY_CURRENT_USER, "HKCU")] {
        for (view, label) in [(KEY_WOW64_64KEY, "64"), (KEY_WOW64_32KEY, "32")] {
            let source = format!("{scope}/{label}");
            match Key::open(root, UNINSTALL, view).and_then(|key| key.children()) {
                Ok(children) => {
                    let mut complete = true;
                    for name in children {
                        ensure!(!cancel.load(Ordering::Relaxed), "Pemindaian dibatalkan");
                        match Key::open(root, &format!("{UNINSTALL}\\{name}"), view) {
                            Ok(key) => {
                                if let Some(app) = registry_application(&key, &name, &source) {
                                    inventory.apps.push(app)
                                }
                            }
                            Err(e) => {
                                complete = false;
                                inventory
                                    .warnings
                                    .push(format!("{source}: satu entri tidak terbaca ({e})"));
                            }
                        }
                        progress(inventory.apps.len());
                    }
                    if complete {
                        inventory.completed_sources.push(source);
                    }
                }
                Err(e) => {
                    // Missing uninstall root in a user/view is a valid empty source.
                    if e.downcast_ref::<windows::core::Error>().is_some_and(|e| {
                        e.code() == windows::core::HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0)
                    }) {
                        inventory.completed_sources.push(source);
                    } else {
                        inventory
                            .warnings
                            .push(format!("{source} tidak terbaca: {e}"));
                    }
                }
            }
        }
    }
    match store_apps(cancel) {
        Ok(apps) => {
            inventory.apps.extend(apps);
            inventory
                .completed_sources
                .push("Store/current-user".into());
        }
        Err(e) => inventory
            .warnings
            .push(format!("Paket Store tidak terbaca: {e}")),
    }
    inventory
        .apps
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    // Same registration mirrored across registry views, never merge distinct keys/locations/versions/scopes.
    let mut seen = std::collections::HashSet::new();
    inventory.apps.retain(|a| {
        seen.insert(format!(
            "{}|{}|{:?}|{:?}|{:?}|{:?}",
            a.source.split('/').next().unwrap_or(""),
            a.id.splitn(3, ':').last().unwrap_or(&a.id),
            a.version,
            a.publisher,
            a.location,
            a.uninstall
        ))
    });
    inventory.scanned_at = chrono::Utc::now();
    Ok(inventory)
}
fn registry_application(key: &Key, name: &str, source: &str) -> Option<Application> {
    let title = key.text("DisplayName")?;
    let product = if key.number("WindowsInstaller") == Some(1) && crate::uninstall::valid_guid(name)
    {
        Some(name.into())
    } else {
        None
    };
    let protected = key.number("SystemComponent") == Some(1)
        || key.number("NoRemove") == Some(1)
        || key.text("ParentKeyName").is_some()
        || key.text("ReleaseType").is_some()
        || title.eq_ignore_ascii_case("RigPulse")
        || [
            "microsoft visual c++",
            "microsoft .net",
            "windows sdk",
            "windows software development kit",
        ]
        .iter()
        .any(|n| title.to_lowercase().starts_with(n));
    Some(Application {
        id: format!("reg:{source}:{name}"),
        name: title,
        publisher: key.text("Publisher"),
        version: key.text("DisplayVersion"),
        installed: key
            .text("InstallDate")
            .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y%m%d").ok()),
        estimated_bytes: key.number("EstimatedSize").map(|n| u64::from(n) * 1024),
        location: key.text("InstallLocation").map(PathBuf::from),
        kind: if product.is_some() {
            AppKind::Msi
        } else {
            AppKind::Desktop
        },
        protected,
        source: source.into(),
        uninstall: key.text("UninstallString"),
        product_code: product,
        package_family: None,
        icon_path: key.text("DisplayIcon").and_then(|s| icon_location(&s)),
    })
}
fn icon_location(text: &str) -> Option<PathBuf> {
    let s = text.trim();
    let s = if let Some(s) = s.strip_prefix('"') {
        s.split('"').next()?
    } else {
        s.rsplit_once(',').map_or(s, |(a, b)| {
            if b.trim().parse::<i32>().is_ok() {
                a
            } else {
                s
            }
        })
    };
    let path = PathBuf::from(s);
    path.is_absolute().then_some(path)
}
fn store_apps(cancel: &AtomicBool) -> anyhow::Result<Vec<Application>> {
    use windows::{ApplicationModel::PackageSignatureKind, Management::Deployment::PackageManager};
    let _apartment = Apartment::new()?;
    let manager = PackageManager::new()?;
    let mut result = Vec::new();
    for package in manager.FindPackagesByUserSecurityId(&HSTRING::new())? {
        ensure!(!cancel.load(Ordering::Relaxed), "Pemindaian dibatalkan");
        ensure!(result.len() < 10000, "Terlalu banyak paket Store");
        let id = package.Id()?;
        let full = id.FullName()?.to_string();
        let version = id.Version()?;
        let location = package
            .InstalledLocation()
            .and_then(|p| p.Path())
            .ok()
            .map(|p| PathBuf::from(p.to_string()));
        let protected = package.IsFramework().unwrap_or(true)
            || package.IsResourcePackage().unwrap_or(true)
            || package
                .SignatureKind()
                .map(|k| k == PackageSignatureKind::System)
                .unwrap_or(true)
            || location.as_ref().is_some_and(|p| {
                p.to_string_lossy()
                    .to_lowercase()
                    .contains("\\windows\\systemapps\\")
            });
        let installed = package
            .InstallDate()
            .ok()
            .and_then(|d| {
                chrono::DateTime::from_timestamp((d.UniversalTime / 10_000_000) - 11_644_473_600, 0)
            })
            .map(|d| d.date_naive());
        let display = package
            .DisplayName()
            .ok()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty() && !s.starts_with("ms-resource:"));
        result.push(Application {
            id: format!("appx:{full}"),
            name: display.unwrap_or(id.Name()?.to_string()),
            publisher: package
                .PublisherDisplayName()
                .ok()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty()),
            version: Some(format!(
                "{}.{}.{}.{}",
                version.Major, version.Minor, version.Build, version.Revision
            )),
            installed,
            estimated_bytes: None,
            location,
            kind: AppKind::Store,
            protected,
            source: "Store/current-user".into(),
            uninstall: Some(full),
            product_code: None,
            package_family: Some(id.FamilyName()?.to_string()),
            icon_path: None,
        });
    }
    Ok(result)
}
pub(crate) fn known_folder(id: &windows::core::GUID) -> anyhow::Result<PathBuf> {
    unsafe {
        let path = windows::Win32::UI::Shell::SHGetKnownFolderPath(
            id,
            windows::Win32::UI::Shell::KNOWN_FOLDER_FLAG(0),
            None,
        )?;
        let value = path.to_string().context("Known folder tidak valid");
        windows::Win32::System::Com::CoTaskMemFree(Some(path.0.cast()));
        Ok(PathBuf::from(value?))
    }
}
pub(crate) fn validate_local_path(path: &Path) -> anyhow::Result<()> {
    use std::path::{Component, Prefix};
    ensure!(path.is_absolute(), "Jalur harus absolut");
    ensure!(
        matches!(path.components().next(),Some(Component::Prefix(p)) if matches!(p.kind(),Prefix::Disk(_))),
        "Hanya jalur drive lokal didukung"
    );
    for c in path.components() {
        if let Component::Normal(s) = c {
            let s = s.to_string_lossy();
            ensure!(
                !s.contains(':') && !s.ends_with('.') && !s.ends_with(' ') && !s.contains('\0'),
                "Komponen jalur tidak aman"
            );
        } else {
            ensure!(
                !matches!(c, Component::ParentDir | Component::CurDir),
                "Traversal ditolak"
            );
        }
    }
    Ok(())
}
