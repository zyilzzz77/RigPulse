# Official application release configuration

RigPulse 0.2.0 has an implemented updater backend and GUI, but the official release service and signing identity have not been provided. `crates/rigpulse-updater/release-config.json` is `null`. This is a fail-closed, visible unavailable state. Do not replace it with an example or third-party repository URL in a shipping build.

## Publisher setup

The publisher must supply an HTTPS URL for the stable signed manifest, an HTTPS origin for MSI packages, and an offline-generated Ed25519 public key (32 bytes, standard base64). The private 32-byte signing seed must be held outside the application/repository, preferably in a protected release secret or offline signing device. Compile only the public key and official locations into the release:

```json
{
  "manifest_url": "<official HTTPS manifest URL>",
  "public_key_base64": "<publisher Ed25519 public key>",
  "package_origin": "<official HTTPS origin, no path/query>"
}
```

Requests use certificate-validated TLS, HTTPS only, no redirects, a 10-second connect timeout and a 120-second per-request timeout. Host/path trust is checked before download. A GitHub Releases deployment must provide a fixed official manifest URL and an exact allowed package origin with direct nonredirecting responses; GitHub's ordinary redirected asset URLs are not supported by this conservative implementation. Use an official HTTPS release endpoint/CDN under publisher control or extend redirect handling with a reviewed allowlist before enabling it.

## Signed manifest protocol

Publish JSON with two base64 strings: `payload_base64` and `signature_base64`. Sign the exact UTF-8 bytes of the payload using Ed25519; no JSON canonicalization occurs. The decoded signed payload follows this schema (placeholder values must be replaced):

```json
{
  "schema": 1,
  "product": "RigPulse",
  "version": "0.3.0",
  "published_at": "<RFC3339 UTC release date>",
  "expires_at": "<RFC3339 UTC expiry date>",
  "notes": "<release notes>",
  "package": {
    "url": "<official HTTPS direct MSI URL>",
    "target": "x86_64-pc-windows-msvc",
    "kind": "msi",
    "size": 12345,
    "sha256": "<64 hexadecimal characters for the exact MSI bytes>"
  }
}
```

Expiry bounds replay of stale release metadata. Re-publish a newly signed manifest before expiry even if the version has not changed. Only stable semantic versions are offered; equal/older versions are not installed. Limits: 256 KiB envelope, 512 MiB MSI. Downloads enforce exact signed size/hash, exclusive partial files, fsync, atomic rename, cancellation and cleanup. After download and before execution, the real MSI database must identify RigPulse, the exact signed version, x64 template and existing UpgradeCode `{E10F4D59-89E4-44BC-BB62-C6C8DE55357F}`. The installer file is locked against writes/replacement while executing.

Create `release.json` from the final signed/distribution MSI size and SHA-256. Set `RIGPULSE_RELEASE_SIGNING_KEY_BASE64` securely in the release environment, then run:

```powershell
cargo run -p rigpulse-updater --example sign_release --locked -- release.json stable.json
```

The utility outputs only the public key and writes the signed envelope. Never print, commit or ship the private seed. Authenticode signing of the MSI/EXE is recommended additionally for Windows publisher identity. SHA-256 alone is not an authenticity guarantee; the updater trusts the cryptographically signed manifest with the embedded public key.

## Installation and verification

The GUI separates available, downloading, verifying, ready, installing, cancelled and failed states. Installation requires explicit confirmation, re-verifies the staged bytes/identity, and invokes the system's `msiexec.exe /i <package> /norestart` with visible Windows Installer UI. No silent elevation or UAC bypass occurs. Success is based on actual exit codes; 1602 is cancellation, 3010/1641 indicates restart, other codes are failures. The current process must be closed/reopened after updating; the UI does not pretend its in-memory version changed.

Before turning on public distribution: validate the publisher endpoint, TLS/offline/timeout behavior, key/signature rotation procedure, cancelled downloads, corrupted/expired manifests, actual old-to-new per-user MSI upgrade, installer failure/recovery and restart behavior in a controlled Windows environment. That online end-to-end verification cannot be claimed until the publisher service and key are configured. Application updater tests run locally with signed test manifests and intentionally corrupted input; test URLs/keys are never production configuration.
