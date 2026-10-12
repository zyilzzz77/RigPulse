use std::process::Command;

fn main() {
    let data = std::fs::read("dist/stable.json").unwrap();
    let r = rigpulse_updater::verify_manifest(
        &rigpulse_updater::ReleaseConfig::bundled().unwrap().unwrap(),
        &data,
        chrono::Utc::now(),
    );
    match r {
        Ok(rel) => {
            let m = rel.metadata();
            println!("OK verified: v{} target={} kind={} size={} sha={}", m.version, m.package.target, m.package.kind, m.package.size, &m.package.sha256[..12]);
            assert_eq!(m.version, "0.4.0");
            assert_eq!(m.package.target, "x86_64-pc-windows-msvc");
            // ensure newer() reports an older install as updatable
            assert!(rigpulse_updater::newer("0.3.1", &m.version).unwrap());
            println!("newer() check passed");
        }
        Err(e) => {
            eprintln!("VERIFY FAILED: {e:#}");
            std::process::exit(1);
        }
    }
}
