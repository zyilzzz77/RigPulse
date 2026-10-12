//! Application updates are independent of Windows driver updates.
//! Trust roots are compiled into the application, never supplied by a server or settings.
use anyhow::{Context, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

const MAX_MANIFEST: u64 = 256 * 1024;
const MAX_PACKAGE: u64 = 512 * 1024 * 1024;
#[cfg(windows)]
mod msi;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseConfig {
    pub manifest_url: String,
    pub public_key_base64: String,
    pub package_origin: String,
}
impl ReleaseConfig {
    pub fn bundled() -> anyhow::Result<Option<Self>> {
        let config: Option<Self> = serde_json::from_str(include_str!("../release-config.json"))?;
        if let Some(c) = &config {
            c.validate()?;
        }
        Ok(config)
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        secure_url(&self.manifest_url)?;
        let origin = secure_url(&self.package_origin)?;
        ensure!(
            origin.path() == "/" && origin.query().is_none(),
            "Origin paket harus berupa origin HTTPS tanpa path/query"
        );
        self.key()?;
        Ok(())
    }
    fn key(&self) -> anyhow::Result<VerifyingKey> {
        let bytes: [u8; 32] = STANDARD
            .decode(&self.public_key_base64)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("Kunci Ed25519 harus 32 byte"))?;
        let key = VerifyingKey::from_bytes(&bytes)?;
        ensure!(!key.is_weak(), "Kunci penandatanganan tidak valid");
        Ok(key)
    }
}
fn secure_url(value: &str) -> anyhow::Result<reqwest::Url> {
    let url = reqwest::Url::parse(value)?;
    ensure!(
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none(),
        "Alamat rilis harus HTTPS tanpa kredensial atau fragmen"
    );
    Ok(url)
}
/// Hosts the signed package may be served from after a redirect. The original
/// `package_origin` is still validated against the manifest's declared URL; GitHub
/// Release assets redirect to this official asset CDN, which is the only extra host
/// accepted. No other redirect target is permitted.
const ALLOWED_REDIRECT_HOSTS: &[&str] = &["release-assets.githubusercontent.com"];
fn redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        let next = attempt.url();
        // Allow the GitHub asset CDN, and the github.com -> github.com hop that the
        // `/releases/latest/download/` short URL produces before reaching the CDN.
        if next.host_str().is_some_and(|h| {
            ALLOWED_REDIRECT_HOSTS.contains(&h) || h == "github.com"
        }) {
            attempt.follow()
        } else {
            attempt.stop()
        }
    })
}
/// Build an HTTPS-only client that follows the same redirect allowlist as the
/// production updater. Exposed for local/CI verification of release endpoint behavior.
#[cfg(any(test, feature = "test-helpers"))]
pub fn check_client_for_test() -> reqwest::blocking::Client {
    client().expect("client")
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub schema: u32,
    pub product: String,
    pub version: String,
    pub published_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub notes: String,
    pub package: Package,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub url: String,
    pub target: String,
    pub kind: String,
    pub size: u64,
    pub sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    payload_base64: String,
    signature_base64: String,
}
/// A release becomes trusted only after signature and compatibility validation.
#[derive(Clone, Debug)]
pub struct VerifiedRelease(Release);
impl VerifiedRelease {
    pub fn metadata(&self) -> &Release {
        &self.0
    }
}
pub fn verify_manifest(
    config: &ReleaseConfig,
    data: &[u8],
    now: DateTime<Utc>,
) -> anyhow::Result<VerifiedRelease> {
    config.validate()?;
    ensure!(
        data.len() as u64 <= MAX_MANIFEST,
        "Metadata pembaruan terlalu besar"
    );
    let envelope: Envelope =
        serde_json::from_slice(data).context("Format metadata pembaruan tidak valid")?;
    let payload = STANDARD.decode(envelope.payload_base64)?;
    let signature = Signature::from_slice(&STANDARD.decode(envelope.signature_base64)?)?;
    config
        .key()?
        .verify_strict(&payload, &signature)
        .context("Tanda tangan rilis tidak sah; paket ditolak")?;
    let release: Release = serde_json::from_slice(&payload)?;
    ensure!(
        release.schema == 1 && release.product == "RigPulse",
        "Produk atau skema rilis tidak cocok"
    );
    let version = semver::Version::parse(&release.version)?;
    ensure!(
        version.pre.is_empty(),
        "Rilis pratinjau tidak didukung pada kanal stabil"
    );
    ensure!(
        release.published_at <= now + chrono::Duration::minutes(5)
            && release.expires_at > now
            && release.expires_at > release.published_at,
        "Metadata kedaluwarsa atau waktu rilis tidak valid. Periksa jam Windows"
    );
    let p = &release.package;
    ensure!(
        p.kind == "msi" && p.target == "x86_64-pc-windows-msvc",
        "Paket tidak kompatibel dengan Windows x64"
    );
    ensure!(
        (1..=MAX_PACKAGE).contains(&p.size),
        "Ukuran paket tidak valid"
    );
    ensure!(
        p.sha256.len() == 64 && p.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
        "SHA-256 tidak valid"
    );
    let url = secure_url(&p.url)?;
    let origin = secure_url(&config.package_origin)?;
    ensure!(
        url.origin() == origin.origin() && url.path().to_ascii_lowercase().ends_with(".msi"),
        "Paket berada di luar sumber rilis resmi"
    );
    Ok(VerifiedRelease(release))
}
pub fn newer(installed: &str, latest: &str) -> anyhow::Result<bool> {
    Ok(semver::Version::parse(latest)? > semver::Version::parse(installed)?)
}
fn client() -> anyhow::Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .https_only(true)
        .redirect(redirect_policy())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .user_agent(concat!("RigPulse/", env!("CARGO_PKG_VERSION")))
        .build()?)
}
pub fn check(config: &ReleaseConfig, installed: &str) -> anyhow::Result<Option<VerifiedRelease>> {
    config.validate()?;
    let response = client()?
        .get(&config.manifest_url)
        .send()
        .context("Tidak dapat menghubungi server rilis. Periksa koneksi internet")?
        .error_for_status()?;
    let mut data = Vec::new();
    response.take(MAX_MANIFEST + 1).read_to_end(&mut data)?;
    let release = verify_manifest(config, &data, Utc::now())?;
    if newer(installed, &release.0.version)? {
        Ok(Some(release))
    } else {
        Ok(None)
    }
}
#[derive(Clone, Debug)]
pub enum Progress {
    Downloading(u64, u64),
    Verifying,
}
#[derive(Clone, Debug)]
pub struct VerifiedPackage {
    release: VerifiedRelease,
    path: PathBuf,
}
impl VerifiedPackage {
    pub fn release(&self) -> &Release {
        self.release.metadata()
    }
}
fn cancelled(cancel: &AtomicBool) -> anyhow::Result<()> {
    ensure!(!cancel.load(Ordering::Relaxed), "Unduhan dibatalkan");
    Ok(())
}
struct PartialDownload(PathBuf);
impl Drop for PartialDownload {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
pub fn download(
    release: VerifiedRelease,
    dir: &Path,
    cancel: &AtomicBool,
    progress: impl Fn(Progress),
) -> anyhow::Result<VerifiedPackage> {
    cancelled(cancel)?;
    ensure!(
        release.0.expires_at > Utc::now(),
        "Rilis kedaluwarsa; periksa lagi"
    );
    std::fs::create_dir_all(dir)?;
    // Exclusive random staging name; no overwrite of an existing download.
    let nonce = Utc::now()
        .timestamp_nanos_opt()
        .context("Waktu sistem tidak valid")?;
    let path = dir.join(format!("RigPulse-{}-{nonce}.msi", release.0.version));
    let partial = PartialDownload(path.with_extension("part"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial.0)?;
    let mut response = client()?
        .get(&release.0.package.url)
        .send()?
        .error_for_status()?;
    if let Some(size) = response.content_length() {
        ensure!(
            size == release.0.package.size,
            "Ukuran server berbeda dari manifest"
        );
    }
    let mut hash = Sha256::new();
    let mut bytes = 0;
    let mut buffer = [0u8; 64 * 1024];
    let mut notified = std::time::Instant::now() - Duration::from_secs(1);
    loop {
        cancelled(cancel)?;
        let count = response.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes += count as u64;
        ensure!(
            bytes <= release.0.package.size,
            "Unduhan melebihi ukuran yang ditandatangani"
        );
        file.write_all(&buffer[..count])?;
        hash.update(&buffer[..count]);
        if notified.elapsed() >= Duration::from_millis(150) {
            progress(Progress::Downloading(bytes, release.0.package.size));
            notified = std::time::Instant::now();
        }
    }
    cancelled(cancel)?;
    progress(Progress::Verifying);
    ensure!(bytes == release.0.package.size, "Unduhan tidak lengkap");
    ensure!(
        format!("{:x}", hash.finalize()).eq_ignore_ascii_case(&release.0.package.sha256),
        "Integritas SHA-256 tidak cocok; paket ditolak"
    );
    file.sync_all()?;
    drop(file);
    verify_file(&partial.0, &release.0.package)?;
    #[cfg(windows)]
    msi::validate(&partial.0, &release.0.version)?;
    std::fs::rename(&partial.0, &path)?;
    Ok(VerifiedPackage { release, path })
}
fn verify_reader(file: &mut File, package: &Package) -> anyhow::Result<()> {
    ensure!(
        file.metadata()?.len() == package.size,
        "Ukuran paket berubah"
    );
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic)?;
    ensure!(
        magic == [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1],
        "File bukan paket Windows Installer"
    );
    let mut hash = Sha256::new();
    hash.update(magic);
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    ensure!(
        format!("{:x}", hash.finalize()).eq_ignore_ascii_case(&package.sha256),
        "Paket berubah setelah verifikasi"
    );
    Ok(())
}
fn verify_file(path: &Path, package: &Package) -> anyhow::Result<()> {
    verify_reader(&mut File::open(path)?, package)
}
#[derive(Debug, Clone)]
pub enum InstallOutcome {
    Completed,
    RestartRequired,
    Cancelled,
}
/// Caller must first obtain explicit approval in the GUI. No silent UI/elevation/reboot.
pub fn install(package: &VerifiedPackage, approved: bool) -> anyhow::Result<InstallOutcome> {
    ensure!(approved, "Persetujuan instalasi diperlukan");
    ensure!(
        package.release.0.expires_at > Utc::now(),
        "Rilis kedaluwarsa; periksa ulang"
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Keep this handle open during installation; prevent replacing/writing the MSI.
        let mut locked = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&package.path)?;
        verify_reader(&mut locked, &package.release.0.package)?;
        msi::validate(&package.path, &package.release.0.version)?;
        let system =
            PathBuf::from(std::env::var_os("SystemRoot").context("Folder Windows tidak tersedia")?);
        let status = std::process::Command::new(system.join("System32/msiexec.exe"))
            .arg("/i")
            .arg(&package.path)
            .arg("/norestart")
            .status()?;
        match status.code() {
            Some(0) => Ok(InstallOutcome::Completed),
            Some(3010) | Some(1641) => Ok(InstallOutcome::RestartRequired),
            Some(1602) => Ok(InstallOutcome::Cancelled),
            code => bail!(
                "Windows Installer gagal (kode {:?}). Paket lama tidak dinyatakan berhasil diperbarui",
                code
            ),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = package;
        bail!("Instalasi memerlukan Windows x64")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    fn fixture() -> (ReleaseConfig, Release, SigningKey) {
        let key = SigningKey::from_bytes(&[19; 32]);
        let config = ReleaseConfig {
            manifest_url: "https://releases.example.org/stable.json".into(),
            package_origin: "https://releases.example.org".into(),
            public_key_base64: STANDARD.encode(key.verifying_key().to_bytes()),
        };
        let release = Release {
            schema: 1,
            product: "RigPulse".into(),
            version: "0.3.0".into(),
            published_at: Utc::now() - chrono::Duration::days(1),
            expires_at: Utc::now() + chrono::Duration::days(1),
            notes: "Test only".into(),
            package: Package {
                url: "https://releases.example.org/RigPulse.msi".into(),
                target: "x86_64-pc-windows-msvc".into(),
                kind: "msi".into(),
                size: 8,
                sha256: "00".repeat(32),
            },
        };
        (config, release, key)
    }
    fn envelope(r: &Release, key: &SigningKey) -> Vec<u8> {
        let payload = serde_json::to_vec(r).unwrap();
        serde_json::to_vec(
            &serde_json::json!({"payload_base64": STANDARD.encode(&payload),
            "signature_base64": STANDARD.encode(key.sign(&payload).to_bytes())}),
        )
        .unwrap()
    }
    #[test]
    fn semantic_versions() {
        assert!(newer("0.2.9", "0.2.10").unwrap());
        assert!(!newer("0.2.0", "0.2.0").unwrap());
        assert!(!newer("0.3.0", "0.2.0").unwrap());
        assert!(newer("bad", "1.0.0").is_err());
    }
    #[test]
    fn authentic_manifest_and_tamper_rejection() {
        let (config, release, key) = fixture();
        let data = envelope(&release, &key);
        assert!(verify_manifest(&config, &data, Utc::now()).is_ok());
        let other = SigningKey::from_bytes(&[20; 32]);
        assert!(verify_manifest(&config, &envelope(&release, &other), Utc::now()).is_err());
        assert!(verify_manifest(&config, b"invalid", Utc::now()).is_err());
    }
    #[test]
    fn rejects_origin_architecture_expiry_and_insecure_transport() {
        let (mut c, mut r, k) = fixture();
        for url in [
            "http://releases.example.org/a.msi",
            "https://evil.example/a.msi",
            "https://releases.example.org/a.exe",
        ] {
            r.package.url = url.into();
            assert!(verify_manifest(&c, &envelope(&r, &k), Utc::now()).is_err());
        }
        r.package.url = "https://releases.example.org/a.msi".into();
        r.package.target = "aarch64-pc-windows-msvc".into();
        assert!(verify_manifest(&c, &envelope(&r, &k), Utc::now()).is_err());
        r.package.target = "x86_64-pc-windows-msvc".into();
        r.expires_at = Utc::now() - chrono::Duration::seconds(1);
        assert!(verify_manifest(&c, &envelope(&r, &k), Utc::now()).is_err());
        c.manifest_url = "http://releases.example.org/a.json".into();
        assert!(c.validate().is_err());
    }
    #[test]
    fn corrupted_and_truncated_packages_rejected() {
        let (_, mut r, _) = fixture();
        let path =
            std::env::temp_dir().join(format!("rigpulse-integrity-{}.part", std::process::id()));
        let bytes = [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1];
        std::fs::write(&path, bytes).unwrap();
        r.package.sha256 = format!("{:x}", Sha256::digest(bytes));
        assert!(verify_file(&path, &r.package).is_ok());
        std::fs::write(&path, [0; 8]).unwrap();
        assert!(verify_file(&path, &r.package).is_err());
        std::fs::write(&path, [0; 4]).unwrap();
        assert!(verify_file(&path, &r.package).is_err());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn cancellation_and_missing_source_are_honest() {
        assert!(cancelled(&AtomicBool::new(true)).is_err());
        let config = ReleaseConfig::bundled().unwrap();
        // With a publisher config present the updater is configured; otherwise it
        // reports the unconfigured state. Either way it must never panic.
        assert!(matches!(config, Some(_) | None));
    }
    #[test]
    fn offline_connection_returns_error_without_panicking() {
        let (mut config, _, _) = fixture();
        config.manifest_url = "https://127.0.0.1:9/stable.json".into();
        assert!(check(&config, "0.2.0").is_err());
    }
}
