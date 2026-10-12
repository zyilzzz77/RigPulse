//! Local/CI verification that the release endpoint is reachable and the signed
//! manifest verifies against the committed public key. Build with:
//! `cargo run -p rigpulse-updater --example check_latest --features test-helpers`.
#![cfg(feature = "test-helpers")]
use std::io::Read;

fn main() {
    let client = rigpulse_updater::check_client_for_test();
    let url = "https://github.com/zyilzzz77/RigPulse/releases/latest/download/stable.json";
    let mut data = Vec::new();
    client
        .get(url)
        .send()
        .expect("fetch")
        .take(256 * 1024)
        .read_to_end(&mut data)
        .expect("read");
    let rel = rigpulse_updater::verify_manifest(
        &rigpulse_updater::ReleaseConfig::bundled().unwrap().unwrap(),
        &data,
        chrono::Utc::now(),
    );
    match rel {
        Ok(r) => println!("OK latest manifest verified: v{}", r.metadata().version),
        Err(e) => {
            eprintln!("VERIFY FAILED: {e:#}");
            std::process::exit(1);
        }
    }
}
