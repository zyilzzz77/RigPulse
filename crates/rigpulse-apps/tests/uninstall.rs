use rigpulse_apps::{
    AppKind, Application,
    uninstall::{Plan, plan, valid_guid},
};
#[test]
fn official_msi_uses_only_valid_product_identity_and_blocks_protected_apps() {
    let mut app = Application {
        name: "Disposable MSI".into(),
        kind: AppKind::Msi,
        product_code: Some("{01234567-89AB-CDEF-0123-456789ABCDEF}".into()),
        ..Default::default()
    };
    assert!(matches!(plan(&app).unwrap(), Plan::Msi { .. }));
    app.protected = true;
    assert!(plan(&app).is_err());
    app.protected = false;
    app.product_code = Some("{bad} /x evil".into());
    assert!(plan(&app).is_err());
    assert!(!valid_guid("{01234567-89AB-CDEF-0123-456789ABCDEZ}"));
}
#[test]
fn shell_strings_relative_paths_and_incomplete_inventory_cannot_authorize_removal() {
    use rigpulse_apps::{
        Inventory,
        uninstall::{parse_command, removal_verified},
    };
    for command in [
        "cmd.exe /c del *",
        "C:\\Windows\\System32\\cmd.exe /c anything",
        "powershell.exe -Command test",
        "uninstall.exe /S",
        "\"C:\\missing.exe",
        "\\\\server\\share\\uninstall.exe /S",
    ] {
        assert!(parse_command(command).is_err(), "{command}");
    }
    let app = Application {
        id: "reg:HKCU/64:missing".into(),
        source: "HKCU/64".into(),
        name: "Example".into(),
        ..Default::default()
    };
    let inv = Inventory {
        apps: vec![],
        scanned_at: chrono::Utc::now(),
        warnings: vec!["denied".into()],
        completed_sources: vec![],
    };
    assert!(!removal_verified(&app, &inv));
}
