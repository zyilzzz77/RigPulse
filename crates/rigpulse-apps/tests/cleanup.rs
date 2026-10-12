use rigpulse_apps::{Application, Inventory, cleanup::*};
use std::sync::atomic::AtomicBool;
fn fixture() -> (tempfile::TempDir, ScanRoots, RemovedApp, Inventory) {
    let base =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/apps-fixtures");
    std::fs::create_dir_all(&base).unwrap();
    let temp = tempfile::tempdir_in(base.canonicalize().unwrap()).unwrap();
    let root = temp
        .path()
        .to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .to_string();
    let base = std::path::PathBuf::from(root);
    let roots = ScanRoots {
        roaming: base.join("Roaming"),
        local: base.join("Local"),
        data: base.join("RigPulse"),
    };
    std::fs::create_dir_all(roots.roaming.join("Code/Cache")).unwrap();
    std::fs::create_dir_all(&roots.local).unwrap();
    std::fs::write(roots.roaming.join("Code/Cache/data_0"), b"temporary cache").unwrap();
    std::fs::write(
        roots.roaming.join("Code/settings.json"),
        b"private preferences",
    )
    .unwrap();
    let app = Application {
        id: "reg:HKCU/64:fixture".into(),
        name: "Microsoft Visual Studio Code (User)".into(),
        publisher: Some("Microsoft Corporation".into()),
        source: "HKCU/64".into(),
        ..Default::default()
    };
    let removed = RemovedApp {
        app,
        verified_at: chrono::Utc::now(),
    };
    let inv = Inventory {
        apps: vec![],
        scanned_at: chrono::Utc::now(),
        warnings: vec![],
        completed_sources: vec!["HKCU/64".into()],
    };
    (temp, roots, removed, inv)
}
#[test]
fn scanner_targets_verified_cache_and_protects_configuration_and_installed_family() {
    let (_temp, roots, removed, mut inv) = fixture();
    let cancel = AtomicBool::new(false);
    let found = scan(
        std::slice::from_ref(&removed),
        &inv,
        &roots,
        false,
        &cancel,
        |_| {},
    )
    .unwrap();
    let safe: Vec<_> = found.candidates.iter().filter(|c| c.eligible).collect();
    assert_eq!(safe.len(), 1);
    assert_eq!(safe[0].path, roots.roaming.join("Code/Cache/data_0"));
    assert_eq!(safe[0].bytes, Some(15));
    assert!(
        !found
            .candidates
            .iter()
            .any(|c| c.eligible && c.path.ends_with("settings.json"))
    );
    inv.apps.push(removed.app.clone());
    assert!(
        scan(&[removed], &inv, &roots, false, &cancel, |_| {})
            .unwrap()
            .candidates
            .is_empty()
    );
}
#[test]
fn cleanup_requires_approval_and_dry_run_preserves_real_fixture_files() {
    let (_temp, roots, removed, inv) = fixture();
    let cancel = AtomicBool::new(false);
    let found = scan(&[removed], &inv, &roots, false, &cancel, |_| {}).unwrap();
    let ids = found
        .candidates
        .iter()
        .filter(|c| c.eligible)
        .map(|c| c.id.clone())
        .collect::<Vec<_>>();
    assert!(
        Quarantine::clean(
            &roots,
            &inv,
            &found.candidates,
            &ids,
            false,
            false,
            &cancel,
            |_| {}
        )
        .is_err()
    );
    let dry = Quarantine::clean(
        &roots,
        &inv,
        &found.candidates,
        &ids,
        true,
        true,
        &cancel,
        |_| {},
    )
    .unwrap();
    assert_eq!(dry.moved, 0);
    assert_eq!(dry.skipped, 1);
    assert!(roots.roaming.join("Code/Cache/data_0").exists());
}
#[test]
fn selected_cache_is_quarantined_and_restored_without_touching_preferences() {
    let (_temp, roots, removed, inv) = fixture();
    let cancel = AtomicBool::new(false);
    let found = scan(&[removed], &inv, &roots, false, &cancel, |_| {}).unwrap();
    let ids = found
        .candidates
        .iter()
        .filter(|c| c.eligible)
        .map(|c| c.id.clone())
        .collect::<Vec<_>>();
    let result = Quarantine::clean(
        &roots,
        &inv,
        &found.candidates,
        &ids,
        true,
        false,
        &cancel,
        |_| {},
    )
    .unwrap();
    assert!(result.failed.is_empty(), "{:?}", result.failed);
    assert_eq!(result.moved, 1);
    assert_eq!(result.bytes, 15);
    assert!(!roots.roaming.join("Code/Cache/data_0").exists());
    assert_eq!(
        std::fs::read(roots.roaming.join("Code/settings.json")).unwrap(),
        b"private preferences"
    );
    let entries = Quarantine::entries(&roots).unwrap();
    assert_eq!(entries.len(), 1);
    Quarantine::recover(&roots, &entries[0].id, true, false).unwrap();
    assert_eq!(
        std::fs::read(roots.roaming.join("Code/Cache/data_0")).unwrap(),
        b"temporary cache"
    );
    assert!(Quarantine::entries(&roots).unwrap().is_empty());
}
#[test]
fn changed_target_cancelled_work_and_protected_selection_are_rejected() {
    let (_temp, roots, removed, inv) = fixture();
    let cancel = AtomicBool::new(false);
    let found = scan(&[removed], &inv, &roots, false, &cancel, |_| {}).unwrap();
    let ids = found
        .candidates
        .iter()
        .filter(|c| c.eligible)
        .map(|c| c.id.clone())
        .collect::<Vec<_>>();
    std::fs::write(roots.roaming.join("Code/Cache/data_0"), b"changed content").unwrap();
    let result = Quarantine::clean(
        &roots,
        &inv,
        &found.candidates,
        &ids,
        true,
        false,
        &cancel,
        |_| {},
    )
    .unwrap();
    assert_eq!(result.moved, 0);
    assert_eq!(result.failed.len(), 1);
    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    let result = Quarantine::clean(
        &roots,
        &inv,
        &found.candidates,
        &ids,
        true,
        false,
        &cancel,
        |_| {},
    )
    .unwrap();
    assert!(result.cancelled);
    assert_eq!(result.moved, 0);
    let unsafe_id = found
        .candidates
        .iter()
        .find(|c| !c.eligible)
        .unwrap()
        .id
        .clone();
    assert!(
        Quarantine::clean(
            &roots,
            &inv,
            &found.candidates,
            &[unsafe_id],
            true,
            false,
            &cancel,
            |_| {}
        )
        .is_err()
    );
}
#[test]
fn hardlinked_cache_and_unknown_publishers_never_become_cleanup_targets() {
    let (_temp, roots, mut removed, inv) = fixture();
    let outside = roots.local.join("private-document");
    std::fs::hard_link(roots.roaming.join("Code/Cache/data_0"), &outside).unwrap();
    let result = scan(
        std::slice::from_ref(&removed),
        &inv,
        &roots,
        false,
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert!(!result.candidates.iter().any(|c| c.eligible));
    assert_eq!(std::fs::read(&outside).unwrap(), b"temporary cache");
    removed.app.publisher = Some("Different Publisher".into());
    assert!(
        scan(
            &[removed],
            &inv,
            &roots,
            false,
            &AtomicBool::new(false),
            |_| {}
        )
        .unwrap()
        .candidates
        .is_empty()
    );
}
#[test]
fn recovery_will_not_overwrite_existing_data_and_permanent_purge_requires_approval() {
    let (_temp, roots, removed, inv) = fixture();
    let cancel = AtomicBool::new(false);
    let scan = scan(&[removed], &inv, &roots, false, &cancel, |_| {}).unwrap();
    let ids = scan
        .candidates
        .iter()
        .filter(|c| c.eligible)
        .map(|c| c.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        Quarantine::clean(
            &roots,
            &inv,
            &scan.candidates,
            &ids,
            true,
            false,
            &cancel,
            |_| {}
        )
        .unwrap()
        .moved,
        1
    );
    let entry = Quarantine::entries(&roots).unwrap().remove(0);
    std::fs::write(roots.roaming.join("Code/Cache/data_0"), b"new data").unwrap();
    assert!(Quarantine::recover(&roots, &entry.id, true, false).is_err());
    assert!(Quarantine::recover(&roots, &entry.id, false, true).is_err());
    Quarantine::recover(&roots, &entry.id, true, true).unwrap();
    assert_eq!(
        std::fs::read(roots.roaming.join("Code/Cache/data_0")).unwrap(),
        b"new data"
    );
    assert!(Quarantine::entries(&roots).unwrap().is_empty());
}

#[test]
fn partial_failure_keeps_changed_file_and_recovery_for_successful_files() {
    let (_temp, roots, removed, inv) = fixture();
    let cache = roots.roaming.join("Code/Cache");
    std::fs::write(cache.join("data_1"), b"another cache").unwrap();
    let cancel = AtomicBool::new(false);
    let scan = scan(&[removed], &inv, &roots, false, &cancel, |_| {}).unwrap();
    let ids: Vec<_> = scan
        .candidates
        .iter()
        .filter(|c| c.eligible)
        .map(|c| c.id.clone())
        .collect();
    assert_eq!(ids.len(), 2);
    std::fs::write(cache.join("data_0"), b"changed after scan").unwrap();
    let result = Quarantine::clean(
        &roots,
        &inv,
        &scan.candidates,
        &ids,
        true,
        false,
        &cancel,
        |_| {},
    )
    .unwrap();
    assert_eq!(result.moved, 1);
    assert_eq!(result.failed.len(), 1);
    assert!(result.failed[0].contains("data_0"));
    assert_eq!(
        std::fs::read(cache.join("data_0")).unwrap(),
        b"changed after scan"
    );
    assert!(!cache.join("data_1").exists());
    let entry = Quarantine::entries(&roots).unwrap().remove(0);
    Quarantine::recover(&roots, &entry.id, true, false).unwrap();
    assert_eq!(
        std::fs::read(cache.join("data_1")).unwrap(),
        b"another cache"
    );
    assert!(Quarantine::entries(&roots).unwrap().is_empty());
}
