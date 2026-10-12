# RigPulse implementation plan

The blueprint has been read in full. Production code never uses synthetic measurements.

- [x] Phase 1: workspace, native dark navy GUI, eight pages, settings, logging, CI. Check/build, 2 tests, strict clippy passed; actual native render captured in artifacts/phase1.png.
- [x] Phase 2 implementation: WMI inventory, native battery IOCTL, copy specs and privacy. Build, 8 tests (including real WMI/power), strict clippy passed on current host. Physical batteryless/cross-brand matrix remains open.
- [x] Phase 3 implementation: CPU/RAM worker, bounded 5/15/60 minute charts, pause, power saver, trusted-path optional NVIDIA NVML. Check/build, 12 tests, strict lint passed. Disk activity/CPU temperature/fan RPM explicitly unsupported pending providers.
- [x] Phase 4 implementation: SetupAPI/CM device scan, metadata/details, filters, WUA read-only search. Build, 16 tests, strict lint; explicit real online WUA test passed in 35.58 seconds without download/install.
- [ ] Phase 5 acceptance: consent UI, state machine, allowlist, WUA download/install and verification are implemented behind `lab-driver-install`. All-feature check and safety tests passed. Production direct installation remains disabled until recoverable lab hardware validation; ordinary users can open Windows Update. No actual driver mutations have been performed.
- [x] Phase 6 implementation: real diagnostics/evidence, transactional SQLite history/audit/retention, private JSON/PDF, export-time Unicode repair. Native 14-page report rendered and text/privacy verified. Final workspace suite: 31 passed, 2 opt-in integrations; strict all-feature lint passed.
- [x] Phase 7 current-host packaging: release x64 GUI EXE, exact user logo/icon/version/manifest, WiX MSI and ZIP, license notices/checksums, real install/GUI/shortcut/upgrade/uninstall test passed; preferences preserved. Physical Windows 10/cross-brand certification and signing remain open.

## Environment

Windows 11 x64 Lenovo host, NVIDIA GeForce RTX 4050 Laptop GPU. Rust 1.90.0, MSVC and Windows SDK provisioned; WiX 3.14.1 portable used for packaging. Generic DXGI/PDH GPU support, NVIDIA NVML metrics, friendly GHz/GiB/GB/Hz/Wh formatting and user-provided logo implemented and checked. Cross-brand and driver mutation tests remain physically untested.
