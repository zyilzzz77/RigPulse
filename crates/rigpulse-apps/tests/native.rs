#![cfg(windows)]
use rigpulse_apps::{AppKind, inventory, uninstall};
use std::sync::atomic::AtomicBool;
#[test]
#[ignore = "Read-only physical Windows inventory, including current-user Store packages"]
fn actual_registry_and_store_inventory() {
    let inventory = inventory::scan(&AtomicBool::new(false), |_| {}).unwrap();
    assert!(inventory.completed_sources.iter().any(|s| s == "HKLM/64"));
    assert!(
        inventory
            .apps
            .iter()
            .any(|a| a.kind == AppKind::Desktop || a.kind == AppKind::Msi)
    );
    assert!(
        inventory
            .completed_sources
            .iter()
            .any(|s| s == "Store/current-user"),
        "{:?}",
        inventory.warnings
    );
    assert!(inventory.apps.iter().any(|a| a.kind == AppKind::Store));
    let summary = serde_json::json!({"total":inventory.apps.len(),"desktop":inventory.apps.iter().filter(|a|a.kind==AppKind::Desktop).count(),"msi":inventory.apps.iter().filter(|a|a.kind==AppKind::Msi).count(),"store":inventory.apps.iter().filter(|a|a.kind==AppKind::Store).count(),"sources":inventory.completed_sources,"warnings":inventory.warnings});
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/apps-native");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("inventory-summary.json"),
        serde_json::to_vec_pretty(&summary).unwrap(),
    )
    .unwrap();
}
#[test]
#[ignore = "Creates a junction only within disposable workspace fixtures to exercise Windows reparse protection"]
fn actual_junction_cache_is_rejected() {
    use rigpulse_apps::{Application, Inventory, cleanup::*};
    use std::os::windows::process::CommandExt;
    let parent =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/apps-junction");
    std::fs::create_dir_all(&parent).unwrap();
    let temp = tempfile::tempdir_in(parent.canonicalize().unwrap()).unwrap();
    let base = std::path::PathBuf::from(
        temp.path()
            .to_string_lossy()
            .trim_start_matches("\\\\?\\")
            .to_string(),
    );
    let roots = ScanRoots {
        roaming: base.join("Roaming"),
        local: base.join("Local"),
        data: base.join("RigPulse"),
    };
    std::fs::create_dir_all(roots.roaming.join("Code")).unwrap();
    std::fs::create_dir_all(&roots.local).unwrap();
    std::fs::write(roots.local.join("private.bin"), b"must survive").unwrap();
    let link = roots.roaming.join("Code/Cache");
    let script = format!(
        "New-Item -ItemType Junction -Path '{}' -Value '{}' | Out-Null",
        link.display().to_string().replace('\'', "''"),
        roots.local.display().to_string().replace('\'', "''")
    );
    let status = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(0x08000000)
        .status()
        .unwrap();
    assert!(status.success());
    let app = Application {
        id: "fixture".into(),
        name: "Microsoft Visual Studio Code (User)".into(),
        publisher: Some("Microsoft Corporation".into()),
        source: "fixture".into(),
        ..Default::default()
    };
    let inv = Inventory {
        apps: vec![],
        scanned_at: chrono::Utc::now(),
        warnings: vec![],
        completed_sources: vec!["fixture".into()],
    };
    let result = scan(
        &[RemovedApp {
            app,
            verified_at: chrono::Utc::now(),
        }],
        &inv,
        &roots,
        false,
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert!(!result.candidates.iter().any(|c| c.eligible));
    assert!(!result.warnings.is_empty());
    assert_eq!(
        std::fs::read(roots.local.join("private.bin")).unwrap(),
        b"must survive"
    );
}
#[test]
#[ignore = "Creates/removes only disposable HKCU uninstall registrations and launches the separately built QA helper"]
fn disposable_official_uninstall_verifies_success_cancellation_and_restart() {
    use windows::{Win32::System::Registry::*, core::PCWSTR};
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    let exe = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/examples/disposable_uninstaller.exe")
        .canonicalize()
        .unwrap();
    let exe = exe
        .to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .to_string();
    struct Fixture(String);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let path = wide(&format!(
                "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{}",
                self.0
            ));
            unsafe {
                let _ = RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr()));
            }
        }
    }
    for code in [0, 1602, 3010] {
        let name = format!("RigPulseDisposableFixture-{}-{code}", std::process::id());
        let _fixture = Fixture(name.clone());
        let path = wide(&format!(
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{name}"
        ));
        let mut key = HKEY::default();
        unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(path.as_ptr()),
                Some(0),
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE | KEY_WOW64_64KEY,
                None,
                &mut key,
                None,
            )
            .ok()
            .unwrap();
        }
        for (property, value) in [
            ("DisplayName", "RigPulse Disposable Test App".to_string()),
            ("DisplayVersion", "1.0.0".into()),
            ("Publisher", "RigPulse QA".into()),
            (
                "UninstallString",
                format!("\"{exe}\" --remove {name} {code}"),
            ),
        ] {
            let data = wide(&value);
            let bytes: Vec<u8> = data.iter().flat_map(|v| v.to_le_bytes()).collect();
            unsafe {
                RegSetValueExW(
                    key,
                    PCWSTR(wide(property).as_ptr()),
                    Some(0),
                    REG_SZ,
                    Some(&bytes),
                )
                .ok()
                .unwrap();
            }
        }
        unsafe {
            RegCloseKey(key).ok().unwrap();
        }
        let inventory = inventory::scan(&AtomicBool::new(false), |_| {}).unwrap();
        let app = inventory
            .apps
            .iter()
            .find(|a| a.id.ends_with(&name))
            .unwrap();
        assert!(uninstall::execute(app, false, &AtomicBool::new(false), |_| {}).is_err());
        let result = uninstall::execute(app, true, &AtomicBool::new(false), |_| {}).unwrap();
        match code {
            0 => assert_eq!(result.outcome, uninstall::Outcome::Removed),
            1602 => assert_eq!(result.outcome, uninstall::Outcome::Cancelled),
            3010 => assert_eq!(result.outcome, uninstall::Outcome::RestartRequired),
            _ => unreachable!(),
        }
    }
}
