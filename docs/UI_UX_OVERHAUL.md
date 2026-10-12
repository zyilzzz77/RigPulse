# RigPulse 0.2.0 — UI/UX and application updater

## Repository audit

The existing Rust 2024 workspace, egui/eframe 0.32.3, glow renderer, Windows API providers and local SQLite architecture were retained. There is no Git repository/remote or configured official release URL in this checkout. The original distribution was 0.1.0, packaged with WiX 3.14.1 as a per-user x64 MSI.

Root layout defects: shrinking Grid columns competed for space; card closures applied incompatible minimum widths; battery grids did not reserve a stable value column; the sidebar switched widths at a breakpoint; monitoring used dense default axes; Drivers rendered every device; sources and long GPU model names dominated primary values. UI labels and version strings were repeated.

Existing providers remain: WMI inventory, sysinfo CPU/memory, DXGI/PDH GPU counters, optional trusted-path NVIDIA NVML, Battery IOCTL, SetupAPI/Configuration Manager, Windows Update Agent, Event Log and storage status. System calls remain on background workers. Direct driver installation remains restricted to the existing laboratory feature.

## Implemented changes

- Shared Navy tokens, Segoe UI, consistent typography/buttons, `surface`, `section`, `row`, `metric`, `paint_icon`, responsive column selection and compact status/empty states.
- Sidebar has fixed 224 logical pixel width, supplied RigPulse logo, aligned outline icons and focusable buttons with selected/hover states. Version comes from Cargo metadata.
- Main vertical scrolling preserves per-page state. Cards use the available content width; specification rows align labels/values and stack below 420 logical pixels. Native minimum window reduced to 620 × 400.
- Dashboard has four CPU/RAM/GPU/battery cards, responsive 4/2/1 columns, device summary and real quick actions. Full GPU model is available in a tooltip and Hardware.
- Monitoring has separate CPU and RAM charts, optional combined view, human time labels, real timestamp hover labels, fixed percentage reference bounds, 5/15/60 minute ranges and separate minimum/average/maximum. Pause freezes a bounded snapshot while background history continues. Unsupported temperatures show unavailable.
- Hardware categories expand/collapse, search is case insensitive across visible names/values, empty results are explained, provider details are collapsed, clipboard follows privacy settings.
- Drivers adds counts/status/check date, categorized filters, detail copy, readable compact rows, and viewport virtualization using the parent scroll area. Official Windows Update integration and driver installation restrictions are retained.
- Battery has charge/progress, readable power/charging/time rows, health/capacity/cycle information and explicit batteryless/unavailable states. Health uses validated firmware capacities and is never confused with charge percentage.
- Diagnostics uses Indonesian evidence/status labels and lists the actual available tests. Reports summarizes which real sources are available before export.
- Settings groups general, monitoring, appearance, privacy and About. Refresh 1/3/5 seconds, default history, power saving, reduced animations, interface scale and application auto-check preference persist. Legacy settings migrate with privacy off. Reset requires confirmation.

## Application updater

`crates/rigpulse-updater` owns HTTPS retrieval, signature validation, semantic version comparison, download/staging, integrity verification, MSI database checks and installer outcome handling. `apps/rigpulse/src/updater.rs` owns background tasks/cancellation/UI state. Application updates appear only under Settings → Tentang RigPulse; driver updates remain under Drivers.

No source URL was invented. `release-config.json` intentionally contains `null`. The shipped interface reports missing release configuration and makes no network request. It does not claim “latest version” or “update successful” without a verified provider result. End-to-end online application updating is **not configured or verified** in this release.

See [APPLICATION_UPDATES.md](APPLICATION_UPDATES.md) for the required official origin/key and publishing protocol. Automatic checks default on with a persisted 24-hour interval when configured. Automatic downloads/install-on-restart are not offered; download is user initiated and installation requires separate approval.

## Changed source files

Cargo.toml, Cargo.lock; apps/rigpulse/Cargo.toml, src/main.rs, app.rs, theme.rs, updater.rs, pages/mod.rs, pages/monitoring.rs, pages/drivers.rs, pages/diagnostics.rs, pages/reports.rs, pages/settings.rs, service.rs; crates/rigpulse-core/src/lib.rs and model.rs; crates/rigpulse-windows/src/battery.rs; crates/rigpulse-monitor/src/lib.rs and buffer.rs; new crates/rigpulse-updater/Cargo.toml, release-config.json, src/lib.rs, src/msi.rs, examples/sign_release.rs; scripts/package.ps1, scripts/test-package.ps1; wix/main.wxs; README.md and documentation.

## Acceptance evidence

Results and remaining manual/hardware coverage are recorded in TEST_PLAN.md after validation. Native render captures are kept under artifacts/gui-v0.2.0. Test data used for adversarial manifests/edge cases is confined to test builds; production hardware values are not fabricated.

## Remaining limits

CPU temperature, fan RPM or GPU temperature can be unavailable depending on supported APIs/driver. AMD/Intel vendor temperature APIs are not added. Other physical GPU models, batteryless hardware, multi-monitor DPI transitions and complete keyboard/screen-reader audits require additional devices/manual testing. Graph history is memory bounded, starts when the app opens, and is not backfilled. The chart reports measured samples without interpolation. Physical recovery testing is still required before direct driver installation can ship.


Drivers follow-up: compact 4/2-column overview, collapsible Windows Update results, full-height search field, six status filters and reset filters. Wide table cells share exact column rectangles; narrow rows place name, category/version and status at a common left edge with a fixed right-side Detail action. Filter widths adapt to the available content space at the 620-pixel minimum window. Fixed 56/92-pixel row allocation prevents virtualization drift. Regression coverage renders 500 entries and checks column boundaries and total row height. Final native captures at 620 pixels and while scrolling at 780 pixels were visually inspected.
