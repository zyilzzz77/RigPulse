# Limitations

- Version 0.4.0 enables direct official WUA download/install after explicit user consent and administrator checks. Recoverable physical installation, UAC denial/acceptance, partial failure and restart acceptance are not performed by automated tests; see COMPLETE_UPGRADE.md. Windows Update remains the fallback for genuine policy/API/license restrictions.
- Windows 11 current Lenovo host is used for real WMI, power/battery, SetupAPI/CM, Event Log, storage health, NVIDIA telemetry and Windows GPU counters. Windows 10, other brands, batteryless desktops, AMD/Intel-only systems and external-GPU hotplug remain physically untested.
- Generic GPU inventory/counters use DXGI/WDDM capabilities. WDDM 2+ counters can be absent/inaccessible/inactive. CPU temperature, generic fan RPM, disk activity, AMD/Intel temperature and raw SMART attributes remain explicitly unavailable. NVIDIA RPM/power/clock can also be unsupported on individual hardware. ACPI thermal zones are never renamed CPU core temperature.
- Graphs cover CPU/RAM, network traffic and optional battery power; GPU readings are live cards. Battery history is also recorded during diagnosis scans. Tray, autostart and complete second-language translation are blueprint P2 and not implemented.
- WUA can take minutes and respects corporate policy. Search runs off the UI thread and exposes WUA cooperative abort; a running Windows operation may take time to stop. WUA may not offer every OEM driver.
- Diagnostics are screening, not proof of complete hardware health. Historical events do not prove current damage. Storage HealthStatus does not substitute for manufacturer SMART testing.
- EXE/MSI are unsigned because no signing certificate is available. Glow rendering requires a working desktop graphics driver.
- Navigation tests exercise logical sizes and 100/150/200% scale settings. Manual multi-monitor DPI and keyboard accessibility acceptance remain open.
- Installer validation and exact results are recorded in TEST_PLAN. No universal hardware/Windows-version certification is claimed.

## Version 0.2.0 update limitations

- The application updater implementation is present, but the official HTTPS release origin and Ed25519 publisher public key are unconfigured. End-to-end online application updating is not available or verified. The interface reports missing configuration and does not claim a successful check/install.
- Application installer execution requires explicit consent and visible Windows Installer. The old driver lab-only restriction was superseded by the explicit 0.4.0 upgrade request; see the current driver limitations above.
- The 0.2.0 layout matrix exercises 1280×720 through 2560×1440 at 100/125/150/175% via egui viewport inputs, plus native narrow-window captures. Physical multi-monitor DPI transitions and full screen-reader/keyboard acceptance remain open.

## Version 0.3.0 Apps Manager

- Inventory covers HKLM/HKCU 32/64-bit uninstall registrations and current-user Store/MSIX packages. Portable apps without registrations, other users' Store apps and reliable last-used times are unavailable. Registry size/date are vendor estimates, not disk measurements. Store package size remains unavailable. Icons use bounded native extraction, with a neutral fallback when unavailable.
- Official MSI/Win32/Store removal is implemented. Actual destructive integration tests use only a disposable Win32 helper; real third-party MSI/Store uninstall and interactive UAC approval/denial remain manual acceptance items. Cancelling monitoring does not kill a vendor uninstaller. Windows or a vendor may request a restart; RigPulse does not restart the machine automatically.
- Only verified local uninstall history is used for leftover scanning. Eligibility currently covers known cache files for VS Code, Discord and Slack with exact publisher/name rules. Unknown installation folders and registry keys are report-only. No generic scan of entire drives or profiles, forced uninstall, registry deletion or shared/system cleanup is provided.
- User data/configuration are never eligible for deletion, even with their report visibility enabled. Junctions, symlinks, hard links, traversal, network paths and changed file identities are rejected. Each scan is bounded to 2,500 candidates, depth 8, 512 MiB of hashing and 64 MiB per file; results can be incomplete and warnings explain skips.
- Quarantine preserves file content on the same volume only, max 2 GiB/2,500 entries. Restore requires the original safe cache parent still to exist and never overwrites a file. Permanent purge has separate confirmation. Retention is manual; moving to quarantine does not reclaim space. A sudden power loss can leave a pending journal entry whose payload is absent; such entries fail recovery explicitly rather than implying success. Whole-folder transactions, cross-volume backup and recovery from user deletion of the journal are not promised.
- Background scans are opt-in and run only while the application is open. No scheduled service, administrator cleanup helper, telemetry or activity tracking is installed.

## Version 0.4.0 networking and power

See COMPLETE_UPGRADE.md for exact Ookla dependency, license/download/cancellation limits, public lookup providers and privacy, optional Wi-Fi permissions, battery sensor handling, WUA privileges and validation evidence. Ookla bandwidth tests are manual and were not run automatically. No fabricated telemetry or successful driver installation is claimed.

