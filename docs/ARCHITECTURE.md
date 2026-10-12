# Architecture

| Module | Responsibility |
| --- | --- |
| apps/rigpulse | eframe window, theme, navigation, worker orchestration |
| rigpulse-core | serializable measurements/capabilities, inventory, battery, drivers, findings, settings |
| rigpulse-windows | WMI, battery IOCTL, SetupAPI/CM, Event Log, DXGI/PDH GPU, fixed shell destinations |
| rigpulse-monitor | bounded CPU/RAM/GPU/battery polling, trusted-path NVML, graph buffer |
| rigpulse-drivers | per-thread COM, WUA search, consent/state machine, lab-only installer |
| rigpulse-diagnostics | conservative evidence-based checks without automatic repair |
| rigpulse-database | SQLite migrations, history, retention/deletion, driver audit |
| rigpulse-updater | HTTPS signed application release manifests, compatibility/integrity checks, staging and Windows Installer |
| rigpulse-apps | Registry/Store inventory, official uninstall, targeted leftover scanner, handle-based quarantine/recovery |
| rigpulse-reports | private snapshot JSON and native Rust paginated PDF, atomic export |

Heavy WMI/COM/diagnosis/export/database calls run on workers. COM connections remain on the creating thread. Owned results cross bounded channels and request GUI repaint. Monitoring drops intermediate samples when rendering falls behind. Graphs retain at most 3600 samples, reject invalid/duplicate timestamps, and pause independently of live counters.

Windows handles use RAII. Variable-length SetupAPI/PDH buffers are aligned and bounded. GPU instances match internal DXGI LUID; utilization sums processes per engine then selects the busiest engine. Memory uses adapter counters rather than summed process memory. Measurements include source, timestamp and availability. Battery health rejects unknown/relative capacities.

WUA respects policy. Firmware/BIOS/critical/unknown classes are excluded. Confirmation binds package ID/revision and expires; installer validates again before download. Production packaging excludes lab capabilities. No arbitrary elevated commands, scraping, force install, automatic EULA acceptance or automatic restart are shipped.

SQLite schema v1 migrates transactionally, retaining 200 diagnosis scans and 365 battery samples. Findings cascade on scan deletion. Private writers remove identifiers at the export boundary. Fonts are read from Windows and embedded in exported PDFs, not shipped as app assets.

Release embeds the user logo, Windows resources, asInvoker manifest and per-monitor DPI support. WiX 3.14.1 creates a per-user x64 MSI with Start Menu shortcut and major-upgrade support. Uninstall preserves user-generated data.

No provider is allowed to substitute sample data when an OS call fails. Sensitive identifiers are excluded by default.

Version 0.2.0 adds a centralized responsive UI and a separate application update controller. Pausing snapshots the bounded graph buffer; the live buffer continues collecting samples. Application release trust roots are compiled from release-config.json, which currently contains null because no official publisher endpoint/key has been supplied. No update network requests occur in this state. See APPLICATION_UPDATES.md and UI_UX_OVERHAUL.md.

Version 0.3.0 adds Apps between Drivers and Battery. A dedicated worker uses one-slot commands and an eight-slot event channel. Windows inventory, hashing, icon extraction and cleanup never execute on the UI thread. Search is debounced; lists render only visible rows. Uninstall approval binds to real inventory, launches the registered executable/MSI/Store API, and verifies the original inventory source before recording removal. Optional UAC retries elevate only the approved official desktop/MSI uninstaller. File cleanup uses narrow publisher rules, reparse/hard-link rejection, SHA-256/file identity revalidation, guarded Windows handles and same-volume renames into a journaled quarantine. Registry deletion is disabled. See APPS_MANAGER.md for boundaries and recovery limits.
