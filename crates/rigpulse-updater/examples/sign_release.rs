//! Release publisher utility. The private signing key never enters application builds.
use anyhow::{Context, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signer, SigningKey};
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.len() == 3,
        "Usage: sign_release release.json envelope.json"
    );
    let secret = std::env::var("RIGPULSE_RELEASE_SIGNING_KEY_BASE64")
        .context("Release signing key is missing")?;
    let seed: [u8; 32] = STANDARD
        .decode(secret)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("Expected a 32-byte Ed25519 seed"))?;
    let key = SigningKey::from_bytes(&seed);
    let payload = std::fs::read(&args[1])?;
    let _: rigpulse_updater::Release = serde_json::from_slice(&payload)?;
    let envelope = serde_json::json!({"payload_base64":STANDARD.encode(&payload),"signature_base64":STANDARD.encode(key.sign(&payload).to_bytes())});
    std::fs::write(&args[2], serde_json::to_vec_pretty(&envelope)?)?;
    println!(
        "Signed manifest written. Public key: {}",
        STANDARD.encode(key.verifying_key().to_bytes())
    );
    Ok(())
}
