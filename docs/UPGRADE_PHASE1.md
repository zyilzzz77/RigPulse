# Complete Upgrade — Phase 1 / RigPulse 0.3.1

## Scope and user acceptance gate

Only Phase 1 of the complete 2,286-line upgrade attachment is implemented in this release. The full attachment was read. Phase 2 must wait for the user's manual test and confirmation that this build behaves normally. Apps popup redesign/uninstall notifications, battery watts, networking, driver ETA/download/install and the final polish phases remain later work.

The existing Rust / egui 0.32.3 / eframe glow / Windows API architecture, Dark Navy theme, logo and Windows data providers are retained. Production uses actual Windows/device data; generated large inventories exist only under `#[cfg(test)]`. Native profiling/screenshot controls require `qa-screenshot`, which is absent from the distributed EXE/MSI/ZIP.

## Findings and changes

Code review plus before/after profiling identified repeated CPU work: Hardware laid out every property, Reports laid out every history card and parsed every timestamp, Drivers rebuilt filters/temporary collections, and the wide Apps details panel deep-cloned the whole filtered inventory. Charts regenerated vectors/statistics on unrelated redraws. The Apps folder button also checked the filesystem during rendering.

- Hardware and Reports render only visible rows plus a small overscan. Cached prefix offsets preserve the full scrollbar extent. Row heights use the same wrapped widgets and are recomputed when data, width, scale, fonts or spacing change; long values stay selectable and readable.
- Hardware privacy/search/category and driver search/category/status filters are cached and invalidated on data/filter changes. List selection and existing page scroll IDs are retained for nonstructural refreshes.
- Apps draws directly from cached indices; opening details no longer clones the collection. Its summary counts/estimated sizes/date are cached per inventory. Folder existence/safety checks and opening run in the existing bounded Apps worker when clicked; a missing folder reports failure.
- Chart points/statistics update on a new sample, selected time window or pause transition. The plot borrows cached series, and hover finds the nearest measured sample by binary search. Pausing still freezes a separate snapshot while real monitoring continues.
- Style mutation is avoided when animation settings have not changed. Existing background providers and bounded channels remain in use. egui's native line/point wheel handling, smoothing and scrollbar behavior are retained; no artificial inertia is added.

Changed implementation: `app.rs`, `apps_service.rs`, `main.rs`, `virtual_rows.rs`, `pages/mod.rs`, `pages/apps.rs`, `pages/drivers.rs`, `pages/reports.rs`, `pages/monitoring.rs`. Development measurements: `perf_tests.rs`, opt-in `qa.rs`. Release integration: Cargo version/lockfile, README, this document, phase/test logs, WiX documentation and package-validation scripts.

## Test boundaries and method

The installed TDD skill was used. The attachment explicitly agrees the observable render/input, wrapped row geometry, filtering and bounded/pause history boundaries. The viewport and wrapping tests were first run red, then implemented and run green; filter/chart and navigation regressions cover the resulting behavior. Startup service events are disconnected in deterministic GUI fixture tests to prevent unrelated background notices from moving click targets; real monitoring stays active in the profiler.

The identical before/after CPU benchmark renders and tessellates 96 cases: four physical resolutions (1280×720, 1366×768, 1920×1080, 2560×1440), four native scale inputs (100/125/150/175%) and six views (Apps, Apps with selection, Drivers, Hardware, Monitoring, Reports). Each case has eight warm-up and 40 recorded frames with repeated high-resolution wheel events. Corpus: 1,000 apps, 500 drivers, 1,000 long hardware properties, 200 history cards and 3,600 chart samples. Actual monitoring is active. Fixtures are excluded from production.

Native line wheel, fractional point wheel and scrollbar dragging are also tested through egui input. A separate real eframe/OpenGL Windows run uses actual device inventory, monitoring and synthetic wheel events. This supplements CPU layout measurements; it does not certify physical touchpad latency or multi-monitor DPI transitions.

## CPU benchmark results

Values below are milliseconds. Mean averages the 16 screen/scale cases for each view; p95 is the worst case p95 among those cases. CPU layout/tessellation timing is not display FPS, GPU time or a promise of smoothness on every laptop.

| View | Release mean before → after | Release worst p95 before → after |
|---|---:|---:|
| Apps | 0.336 → 0.323 | 0.919 → 0.766 |
| Apps with selection | 0.427 → 0.294 | 1.146 → 0.643 |
| Drivers | 0.310 → 0.288 | 0.731 → 0.741 |
| Hardware | 3.337 → 0.301 | 6.673 → 0.633 |
| Monitoring | 0.340 → 0.168 | 0.620 → 0.340 |
| Reports | 1.087 → 0.149 | 1.947 → 0.274 |

Debug mean before → after: Apps 6.360 → 5.433; Apps with selection 6.761 → 5.639; Drivers 5.585 → 4.951; Hardware 65.910 → 6.474; Monitoring 6.143 → 3.444; Reports 20.835 → 2.958 ms. The release baseline was already below 16.7 ms in the CPU benchmark. Hardware/Reports show the clearest improvement. The small Drivers p95 change is within run-to-run variation and is not presented as a measured responsiveness gain.

Raw reports: `artifacts/performance-phase1/{before,after}-{debug,release}.json`. Native evidence and screenshots are recorded in that artifact directory. First-open scan and layout invalidation costs are outside warmed frame timings; a width/scale/filter change measures the relevant rows once.

## Native Windows validation

Native release runs used 1366×768 logical window requests, ten seconds of warm-up and 180 measured frames per view. Windows inventory reported 300 drivers, 143 hardware properties and, on Apps, 505 registrations/packages (182 regular apps visible under the default protected-component filter). Monitoring used actual device counters.

| View | UI CPU mean before → after (ms) | Native frame-interval p95 before → after (ms) |
|---|---:|---:|
| Apps | 0.367 → 0.420 | 6.779 → 6.886 |
| Drivers | 0.368 → 0.370 | 6.804 → 6.687 |
| Hardware | 0.816 → 0.349 | 6.940 → 6.804 |
| Monitoring | 0.244 → 0.300 | 6.691 → 6.802 |
| Reports | 0.146 → 0.228 | 6.795 → 6.783 |

Small native Apps/Monitoring/Reports workloads showed run-to-run variation rather than a uniform timing gain; the clear native CPU reduction was Hardware.

Frame intervals reflect scheduling/presentation between application updates, not physical input latency. After-run maximum intervals included isolated 20–29 ms spikes while GUI CPU work stayed below 1 ms on those pages; the result does not establish that every presented frame meets 16.7 ms. User acceptance on the target laptop remains required. Native captures of narrow/scrolled Hardware, scrolled Drivers, wide Apps details, Monitoring and Reports were visually inspected. Large Reports histories are covered by the generated test corpus; the host's native Reports capture has no stored scan history.

Formatting and strict all-target/all-feature Clippy passed. The workspace suite passed 62 tests; an additional nonstructural-refresh/scroll-state regression passed afterward (63 total distinct passing tests). Seven opt-in tests are excluded by default, including the profiler; the profiler was run explicitly in both configurations. MSI identity/payload validation is recorded separately in TEST_PLAN.md.

## Reproduce measurements

With the local toolchain loaded using `scripts/dev-env.ps1`:

```powershell
$env:RIGPULSE_PERF_LABEL='manual'
cargo test -p rigpulse scroll_frame_profile --locked -- --ignored --nocapture
cargo test -p rigpulse --release scroll_frame_profile --locked -- --ignored --nocapture
```

Native QA only: build with `--features qa-screenshot`; set `RIGPULSE_QA_PAGE`, `RIGPULSE_QA_SIZE` (logical window size) and `RIGPULSE_QA_PERF` (absolute JSON path), then launch. After data is ready and ten seconds of warm-up, 180 frames are measured and the QA window closes. Clear screenshot/scroll/detail environment overrides first. Rebuild with default features before packaging.

## Manual acceptance on the user's laptop

1. Install `RigPulse-0.3.1-x64.msi`, then open RigPulse and check version 0.3.1 in Settings. The portable ZIP can be extracted and run without installation.
2. Keep monitoring active. Scroll Apps, Drivers, Hardware and Reports quickly up/down; also try small wheel/touchpad movements and drag the scrollbar.
3. Search/filter Apps and Drivers, open/close details, change Hardware categories, and confirm values/selection remain correct. This phase does not require uninstalling an app or installing a driver.
4. Resize/maximize the window and try Windows display scales 100/125/150/175% where available. Check long hardware values wrap without overlapping or disappearing.
5. Switch chart time windows; pause/resume. A paused chart stays fixed while live readings continue.
6. Report the page/action if anything freezes, jumps unexpectedly or overlaps. Phase 2 begins only after the user confirms this Phase 1 test build is normal.

The package remains unsigned because no publisher certificate is configured. The existing user's installed copy is not replaced by our smoke tests; package extraction and a separate GUI launch verify the payload.
