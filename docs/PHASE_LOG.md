# Phase reports

## Complete Upgrade — Phases 2–6 / 0.4.0, 12 October 2026

The user approved continuing all remaining phases after the 0.3.1 performance work. This supersedes the historical Phase 1 acceptance gate and lab-only driver statements below.

- Phase 2: centered, viewport-constrained Apps details and confirmations; verified uninstall refresh/notifications; separate advanced-cleanup review; preserved search/filter/sort.
- Phase 3: signed real battery-status power in W, voltage/chemistry, optional bounded history, truthful unavailable/zero handling and unsupported-query backoff.
- Phase 4: native interface counters, route selection, 5/15/60-minute graph, pause, actual traffic totals, native IPv4/IPv6 ping, opt-in public IP/ISP/ASN and manual Wi-Fi identity. The user's explicit Ookla request replaced the earlier prototype with a Rust wrapper around the official Ookla CLI. Its separate first-use download requires agreement; the proprietary binary is not bundled in the installer.
- Phase 5: asynchronous official WUA search/download/install with measured progress/ETA, fresh package validation, restricted administrator manager, cooperative cancellation, results/audit/refresh and separately confirmed restart. Direct installation is enabled in production after explicit confirmation. Automated validation does not install real drivers or reboot the host.
- Phase 6: shared expiring/dismissible notices, persisted new preferences, bounded workers/history, Network plot cache, responsive layout validation and the Windows release/MSI workflow. Full details, changed areas, data sources and practical limits: COMPLETE_UPGRADE.md.
- Default workspace suite: 73 passed, 0 failed, 9 opt-in tests excluded. The subsequently added network-close regression and all 18 application tests also passed, bringing the distinct default total to 74. Formatting and strict all-target/all-feature Clippy passed after the final shutdown change. Separately, three native Apps checks, physical GPU counters, native network, official Ookla archive extraction, read-only asynchronous WUA search and MSI identity/version rejection passed: 8 opt-in integrations. No real Ookla bandwidth test is run without the user's runtime agreement.
- Optimized default-feature Windows x64 GUI build, embedded 0.4.0 version, WiX MSI and portable ZIP packaging passed. Administrative extraction verified the payload hash/version/docs, and the extracted production GUI launched/responded/closed normally with preferences preserved. Existing user installation was not changed. Package/smoke evidence: artifacts/upgrade-package.txt and artifacts/package-test-0.4.0/result.txt. Signing, destructive driver and broader physical acceptance limits remain explicit in COMPLETE_UPGRADE.md.

Earlier phase records below describe their stated releases and remain historical evidence.

## Phase 1 — Foundation & GUI

- Implemented: Cargo workspace, native eframe window, Dark Navy tokens, outline vector icons, eight navigation destinations, persisted preferences, local structured logs, startup error dialog, Windows CI.
- Files changed: root Cargo/toolchain configuration; apps/rigpulse; crates/rigpulse-core; scripts/dev-env.ps1; .github/workflows/ci.yml; README, PLAN and docs.
- How to run/test: `cargo run -p rigpulse`; `cargo check --workspace`; `cargo test --workspace`; `cargo clippy --workspace --all-targets -- -D warnings`.
- Tests passed: check, native debug executable build, 2 unit tests, strict clippy, actual eframe screenshot smoke test.
- Known limitations: physical keyboard/DPI matrix remains manual; hardware pages explicitly pending at this milestone.
- Next: Phase 2 real hardware and battery providers.

## Technical references verified

## Phase 2 — Hardware & Battery Basics

- Implemented: asynchronous inventory, twelve hardware categories plus firmware mode, native battery power and capacity IOCTL, per-pack health validation, search/category filter, copy specifications and sensitive-field opt-in.
- Files changed: rigpulse-windows inventory/battery and probe example; rigpulse-core models; app service and pages.
- How to run/test: standard workspace build/test; `cargo run -p rigpulse-windows --example probe -- artifacts/phase2-real.json` produces a private real-device snapshot.
- Tests: check/build, 8 unit/integration tests including actual WMI and Windows power calls, strict clippy passed. Floating-point test assertion was corrected to use tolerance.
- Limitations: physical desktop without battery, other brands and Windows 10 still require testing. GPU VRAM intentionally omitted from 32-bit WMI AdapterRAM because it is not reliable for large adapters.
- Next: Phase 3 worker-driven monitoring.

## Phase 3 — Live Monitoring

- Implemented: worker-driven real CPU/RAM counters, time-based 5/15/60 minute graph, bounded 3600-sample ring, min/average/max, graph pause, normal/power-saver refresh, stale metadata and optional NVIDIA NVML loaded by absolute trusted paths.
- Files changed: rigpulse-monitor provider/buffer; app monitoring page and live dashboard cards.
- How to run/test: workspace tests/build; open Monitoring and pause/resume graph.
- Tests passed: check/build, 12 tests including real CPU/RAM and bounded/deduplicated/invalid/time-window buffer cases; strict lint.
- Limitations: CPU temperature, RPM and disk activity report unsupported. AMD/Intel telemetry is unsupported. No global temperature safety threshold is inferred.
- Next: native device inventory and online WUA read-only search.

## Phase 4 — Driver Scanner

- Implemented: SetupAPI enumeration, CM problem/status normalization, driver version/provider/date, searchable class and issue filters, private device details, WUA driver-only search and Windows Update launcher.
- Files changed: core driver models; Windows devices/shell; rigpulse-drivers WUA; driver UI/service.
- Tests passed: build/check, 16 local tests, strict lint; explicit online read-only WUA test passed in 35.58 seconds.
- Limitations: Windows Update may not offer every OEM driver. Unknown/policy-restricted information remains unavailable.
- Next: isolated installer safety workflow.

## Phase 5 — Driver Installer (implementation, acceptance pending)

- Implemented: separate lab-only WUA download/install, fresh ID/revision and class validation, explicit consent token with expiry, state transition checks, confirmation GUI, real indeterminate progress, per-package HRESULT/restart result, post-install SetupAPI verification.
- Tests passed: all-feature compile and 19 local tests including no-consent, invalid order, stale/untrusted/firmware rejection. No actual download or install was executed.
- Files changed: rigpulse-drivers/install.rs; app driver confirmation/service; database operation audit methods.
- How to run/test: normal workspace test/lint plus `cargo check --workspace --all-features`. `lab-driver-install` is for recoverable hardware lab validation only.
- Production gate: direct installation disabled until tested on recoverable lab hardware. The normal release provides Open Windows Update, preserving Windows' own privilege/policy flow. No arbitrary elevated helper is shipped.
- Limitations: lab build requires Windows privilege; denied access reports failure and directs to Windows Update. Restore-point creation/rollback are not promised. Physical install/UAC-denial/restart validation is outstanding.
- Next: independent diagnostics/reports and packaging can be implemented and tested while lab acceptance remains open.

- [eframe 0.32.3](https://docs.rs/eframe/0.32.3/eframe/): native App::update and run_native.
- [sysinfo 0.37.2](https://docs.rs/sysinfo/0.37.2/sysinfo/): refresh and memory/CPU APIs.
- [WMI 0.18.4](https://docs.rs/wmi/0.18.4/wmi/struct.WMIConnection.html): per-thread WMI connections.
- [Microsoft battery information](https://learn.microsoft.com/en-us/windows/win32/power/battery-information-str): capacity flags, unknown sentinel and cycle limitations.
- Windows bindings are verified directly against the downloaded windows-rs 0.62.2 generated Rust source.

## Phase 6 — Diagnostics, Database & Reports

- Implemented: real read-only device/Event Log/storage/battery diagnosis, evidence and recommendations, transactional SQLite migration/scans/battery/audit, scan loading/deletion/retention, sensitive-field opt-in, atomic JSON/PDF exports.
- Files changed: rigpulse-diagnostics, rigpulse-database, migrations, rigpulse-reports, app diagnostics/reports/services.
- How to run/test: `cargo test --workspace --locked`; strict all-feature clippy; `cargo run -p rigpulse-reports --example qa_export -- artifacts/qa-report`.
- Tests passed: final 31-test workspace suite; separate real WUA and physical DXGI/WDDM integrations passed. Real 14-page A4 PDF rendered on every page; searchable text, units, nonempty export, private JSON identifiers and database roundtrip verified. Regression test covers correct UTF-16 non-BMP CMap encoding; SQLite tests cover failed-transaction rollback and cascade retention/deletion.
- Known limitations: raw SMART, verified CPU/fan sensors and sustained-usage diagnosis remain explicitly unavailable; Event Log is screening rather than proof of hardware damage.
- Next steps: physical cross-brand matrix and driver lab acceptance remain outstanding; packaging is recorded in Phase 7.

## Additional requested GPU/UI/logo work

- Implemented: vendor-neutral DXGI inventory and WDDM/PDH utilization/dedicated/shared memory; per-adapter matching; optional NVIDIA RTX/GTX/Quadro NVML temperature/VRAM/power/clock/RPM; no GPU configuration writes. GPU readings visible on Dashboard and Monitoring.
- Friendly inventory: GHz clocks, GiB memory, GB disk capacity, Hz refresh, Wh battery capacities; Indonesian names, connection/status labels and unavailable values. Provider source remains visible.
- Branding: exact user-provided `RigPulse Neon Monitoring Logo.png` embedded; ICO prepared in Rust at build time; native window and MSI shortcut use it.
- Files changed: windows/gpu.rs, windows/friendly.rs, inventory, monitor, app pages/theme/main/build resources, package metadata.
- Tests: actual host RTX 4050 metrics/counters passed; multi-adapter aggregation/invalid counters/unit formatting covered; all eight routes clicked at supported logical widths and 100/150/200% scale settings. Actual native render captured for eight pages.
- Limitations: AMD/Intel/other model physical matrix remains open; sensor capabilities depend on the driver.

## Phase 7 — Windows Release & Installer

- Implemented: optimized x64 Windows GUI EXE, logo/icon/version/asInvoker/DPI manifest, WiX 3.14.1 per-user MSI, Start Menu shortcut, major upgrades, uninstall, portable ZIP, third-party notices and SHA-256 sums; Windows CI builds/packages/uploads artifacts.
- Files changed: app build.rs/manifest/Cargo metadata, wix/main.wxs/license.rtf, scripts/package.ps1, scripts/test-installer.ps1, CI and release documentation.
- How to run/test: `./scripts/package.ps1 -WixBin <WiX3.14.1-folder>`; `./scripts/test-installer.ps1 -Msi dist/RigPulse-0.1.0-x64.msi -UpgradeMsi <QA-upgrade-msi>`. Test refuses to replace an existing application installation.
- Tests passed: release build, WiX MSI compilation/link/ICE validation (ICE91 warnings are expected for intentionally per-user-only files), real installation exit 0, exact installed EXE checksum, shortcut, GUI launch/respond/normal close, major upgrade 0.1.0 to 0.1.1 QA package with old registration removed/new installed, uninstall exit 0, executable/shortcut removal and preference hash preservation. Test installation was removed afterward. The upgrade test package is QA-only; shipped version remains 0.1.0.
- Known limitations: unsigned (no certificate), physical matrix and manual DPI/accessibility remain open. Driver installation remains lab-only as documented in Phase 5.
- Next steps: recoverable driver hardware acceptance and broader Windows/vendor matrix; optional blueprint P2 features.

## Follow-up release 0.2.0 — UI/UX and application updater

Implemented the attached overhaul task while retaining the Rust/egui/Windows provider architecture. Priority fixes cover battery wrapping, shared card/row geometry, responsive dashboard, simpler charts, categorized hardware, virtual driver table with a subsequent alignment correction, grouped settings and the application update controller/backend. Version/publisher/license use Cargo metadata. No official release URL or key was invented; production reports updater configuration missing. Details and changed-file inventory: UI_UX_OVERHAUL.md. Publisher setup: APPLICATION_UPDATES.md. Current validation: TEST_PLAN.md. Original direct driver installation lab gate remains in effect.

## Follow-up release 0.3.0 — Apps Manager, uninstall and cleanup

Implemented the complete Apps attachment in its eight stages: foundation/navigation, real inventory, official uninstall, targeted scanner, approved cleanup/recovery, independent cleaner, settings, and UI/release integration. A new rigpulse-apps crate isolates Windows providers and mutation boundaries; the app uses a dedicated bounded worker, virtualized lists and responsive details. The user's later administrator request is implemented as explicit UAC retry of the approved official desktop/MSI uninstaller.

Stage checks included test-first search/MSI/scanner/approval cases, iterative crate/application builds, real isolated quarantine/recovery tests, final strict all-target/all-feature lint, 56 passing workspace tests and native integrations. Native read-only inventory reported 506 registrations/packages with complete sources. Disposable official Win32 uninstall verified success/cancel/restart outcomes; actual junction protection passed. Physical GPU counters and read-only Windows Update regression tests also passed. No user application or driver was removed. Final MSI payload/native GUI results are recorded in TEST_PLAN.md.

Known boundaries: cleanup eligibility is limited to known VS Code/Discord/Slack cache rules after verified local removal; registry/unknown installation folders are report-only. User data is always protected. Quarantine uses manual retention and same-volume moves. Third-party MSI/Store uninstall and interactive UAC approval/denial still need manual acceptance on a disposable machine. The existing updater publisher configuration and driver lab gate are unchanged. Architecture, settings, source details, safety/recovery limits, changed files and reproduction commands: APPS_MANAGER.md.

## Complete Upgrade — Phase 1 only, 0.3.1

Implemented the performance phase of the complete upgrade attachment: visible-row Hardware/Reports rendering with cached wrapped geometry, cached driver/Hardware filtering, Apps summaries/direct-index lists and removal of full-inventory detail clones, asynchronous install-folder opening, cached borrowed chart series/statistics/hover search, and conditional style updates. The installed TDD skill guided viewport/wrapping tests; input/filter/chart regressions and debug/release before/after profiles validate the changes. Details, raw measurements and manual acceptance instructions: UPGRADE_PHASE1.md. Phase 2 is explicitly gated on the user's test confirmation.
