# RigPulse Apps Manager — version 0.3.0

Implemented from the complete Apps Manager / Advanced Uninstaller / Leftover Cleaner attachment, plus the user's administrator-permission request. The application remains native Rust, egui/eframe and Windows API. Production inventory and outcomes are real; fixtures are confined to tests and an undistributed test helper.

## Features and user flow

Apps appears between Drivers and Battery. Its three tabs are Aplikasi Terinstal, Uninstall Lanjutan and Pembersih Sisa Aplikasi. Summary cards explain app count, estimated known sizes, leftover candidates and scan time. Search matches name/publisher without case sensitivity; filters cover desktop, Store, recent registrations, large apps, unknown size and an explicit protected/system inspection view. Sorting uses name, publisher, known size or installation date. Unavailable last-used times are labelled honestly rather than inferred from unrelated timestamps.

Virtualized rows show application name/version, publisher, size and details. Wide views show a details panel; narrow views use a bounded scrollable details window. Details include real source/metadata, copy and safe open-folder actions. Long values truncate with tooltips or wrap in details. Icons are read from existing Windows resources off the UI thread; a neutral grid is used if extraction is unavailable.

Official uninstall always needs confirmation. Advanced uninstall follows the same official method, then optionally scans leftovers after removal is verified. Nothing is selected for cleanup automatically. Users review per-file category, size, confidence and risk; preview performs validation without mutation. Approved cleanup moves eligible selected cache files into quarantine. Restore and permanent purge each have a separate confirmation. Quarantine is paginated rather than silently omitting older entries.

## Sources and architecture

`rigpulse-apps` owns provider, query, uninstall, scanner and quarantine boundaries. `apps_service.rs` orchestrates a dedicated worker with a one-slot command queue and eight-slot event channel. Provider calls, COM/WinRT initialization, hashing, icons, file operations and serialization run on that worker. Cancellation is checked at bounded operation boundaries. Search debounces 250 ms and filtering is cached; app and candidate lists render only visible rows.

Inventory enumerates HKLM/HKCU uninstall keys in both 64-bit and 32-bit views. It reads DisplayName, publisher, version, install date/location, EstimatedSize, DisplayIcon, uninstall information and system/removal flags. Duplicate registry views are merged conservatively; different scopes/versions/locations remain distinguishable. Missing roots are empty sources; unreadable sources are flagged and cannot establish verified removal. No Win32_Product query is used.

Store/MSIX inventory uses Windows.Management.Deployment.PackageManager for the current user. Package full/family identity, display name, publisher, version, installation date and location come from Windows. Framework/resource/system packages are protected. Store sizes are unavailable. Portable apps without registrations and packages belonging to other users are outside inventory scope. Size/date values supplied by installers remain labelled estimates.

## Uninstall and administrator permission

Win32 removal parses only a validated absolute EXE boundary, preserves vendor arguments and invokes ShellExecuteEx directly. Shell/script launchers are rejected; no cmd/PowerShell command is constructed. MSI uses the trusted Windows system msiexec with a validated product GUID and visible `/x ... /norestart`. Store removal uses RemovePackageAsync for the current user. Protected/shared runtimes and RigPulse itself cannot be removed through this feature.

Immediately before launch, inventory is refreshed and metadata must still match the approved app. RigPulse waits for the real process or package operation. Exit code zero alone is insufficient: the original source must be completely readable and the app family absent before removal is recorded. Cancellation stops monitoring without terminating an external vendor process. Restart outcomes and unverified pending operations are persisted and reconciled on refresh; no automatic restart occurs.

Access-denied/elevation-required errors offer a separate administrator retry for the same desktop/MSI uninstaller. Confirmation explains UAC, then ShellExecuteEx uses `runas`. Only that uninstaller is elevated. There is no globally elevated RigPulse process or privileged cleanup service. Store removal remains a current-user operation. If Windows/vendor policy refuses removal, the actual failure remains visible.

## Scanner and protections

Scanning uses only locally recorded, verified removals and rechecks current inventory. Known cache rules currently support exact Visual Studio Code/Microsoft, Discord/Discord Inc. and Slack/publisher combinations. Only the rule's Cache, Code Cache or GPUCache subtrees are inspected. Unknown apps can have their former installation folder reported without enabling removal. Registry review, when enabled, inspects only exact known HKCU keys and is report-only; registry deletion is disabled because reliable rollback is not implemented.

Configuration and personal files are never eligible, including documents, images, projects, scripts, executables and databases. Shared paths, Windows/system locations, RigPulse's executable/data, installed app families, network paths, path traversal, alternate streams, junctions/symlinks, hard links and changed identities are rejected. Safe current-user known folders are resolved through Windows APIs. A scan is capped at 2,500 candidates, depth 8, a 512 MiB hashing budget and 64 MiB per file; skips and incomplete coverage are reported. Unknown ownership means report-only, not guessed deletion.

Cleanup revalidates approval, selected IDs, the rule, inventory, path and a sealed snapshot of volume/file identity, size, modification time and SHA-256. Guarded Windows handles hold ancestor and target identities; reparse points and multiply linked files are refused. Files are moved individually with same-volume handle-based rename. There is no recursive delete of an application directory. Changed/busy/restricted files are skipped with per-item outcomes; cancellation preserves completed work and its recovery records.

## Recovery, settings and limits

`%LOCALAPPDATA%/RigPulse/apps/history.json` keeps up to 256 verified removals and 32 pending operations. `apps/quarantine/` contains individually renamed files; `apps/quarantine.json` is the bounded recovery journal. The journal is written before each move using synced temporary files and atomic replacement. Quarantine caps at 2 GiB/2,500 entries and keeps files until explicit restore or purge. Moving files into quarantine does not itself free disk space.

Restore verifies the quarantine fingerprint, original narrow cache rule and existing safe parent; it never overwrites an existing destination. Cross-volume moves and recreation of missing app directory trees are refused. Purge verifies the payload and deletes through its handle after confirmation. A power interruption can leave a pending record with an absent payload; recovery fails explicitly for that item. There is no promise of whole-folder transaction recovery or repair after a user manually deletes the recovery journal.

Settings persist with backward-compatible defaults: scan after advanced uninstall ON, background scan OFF, preserve/hide personal configuration reports ON and registry review OFF. User data remains protected even when its report visibility changes. Background scans occur every 30 minutes only while RigPulse is open and only when enabled. Retention is deliberately manual; no unreliable timer-based deletion is exposed.

## Phase and test evidence

The attachment explicitly authorizes tests of inventory/search, official uninstall handling, ownership/path protection, approved cleanup, recovery and integrated UI. These are the agreed public test seams for the TDD workflow; real destructive tests use only isolated workspace fixtures. Initial failing tests for search, MSI planning, scanner rules and approval/dry-run became passing tests after implementation.

| Phase | Implementation and validation |
| --- | --- |
| 1 — Foundation | Workspace/theme audit, nine-page navigation, Apps route and bounded controller; application builds and navigation checks pass. |
| 2 — Inventory | Registry/current-user Store providers, metadata, dedup, query and icons; read-only native integration reports 506 entries (126 desktop, 241 MSI, 139 Store), all five sources complete, no source warnings. |
| 3 — Official uninstall | Native EXE/MSI/Store methods, consent, verification, cancellation, persisted pending restart and UAC retry; planning/unsafe-command tests pass. Disposable native helper exercises exit 0, cancellation 1602 and restart 3010 through the actual launch/wait path. |
| 4 — Scanner | Narrow publisher rules, inventory ownership, cache-only eligibility, bounded scans and link protection; real fixture tests pass, including actual Windows junction rejection. |
| 5 — Cleanup/recovery | Dry-run, approved selected moves, quarantine, fingerprint revalidation, restore/no-overwrite and permanent purge; real isolated file tests pass. |
| 6 — Independent cleaner | History-based standalone scan, per-item review, candidate virtualization, skip/partial/cancel and paginated recovery UI; integrated application build passes. |
| 7 — Settings | Four persisted settings, migration/default tests, real behavior and manual retention; settings tests pass. |
| 8 — UI/release | Full 56-test workspace suite passes; strict all-target/all-feature Clippy passes. Nine pages render across 1280×720, 1366×768, 1600×900, 1920×1080 and 2560×1440 at 100/125/150/175% software viewport DPI. Pointer tests switch all three Apps tabs with 1,000 fixture rows at narrow/wide sizes and assert wide details viewport geometry. Native captures and final installer results are recorded in TEST_PLAN.md. |

Workspace fixture tests exercise approval refusal, dry-run non-mutation, changed-target refusal, cancellation, hard-link refusal, unknown publisher, preservation of preferences, quarantine/restore, no overwrite, approved purge and recovery after partial failure. The UI exposes skipped-file details with a bounded first-64 list, while complete failure details remain in the local log. Read-only inventory and disposable Win32 uninstall integration passed on the actual Windows 11 host. Real third-party MSI/Store removal, interactive UAC approval/denial, Windows 10, multi-monitor transitions and full accessibility acceptance remain manual validation items; they are not claimed as tested.

## Main changed files

- New crate: `crates/rigpulse-apps/{Cargo.toml,src/{lib,model,query,native,inventory,icons,uninstall,safe_fs,cleanup,history}.rs,tests/{inventory,uninstall,cleanup,native}.rs,examples/disposable_uninstaller.rs}`.
- Integration: `apps/rigpulse/src/{apps_service,app,main,qa,theme}.rs`, `pages/{mod,apps,settings}.rs`, app Cargo manifest and core navigation/settings.
- Release/docs: workspace manifest/lock, `scripts/{package,test-package}.ps1`, `wix/main.wxs`, README and architecture/limitations/test/phase documents.

## Build and reproduce

On Windows x64 with the Rust MSVC toolchain, C++ Build Tools, Windows SDK and WiX 3.14.1:

```powershell
. ./scripts/dev-env.ps1
$env:CARGO_INCREMENTAL='0'
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
cargo test -p rigpulse-apps --test native actual_registry_and_store_inventory --locked -- --ignored
cargo test -p rigpulse-apps --test native actual_junction_cache_is_rejected --locked -- --ignored
cargo build -p rigpulse-apps --example disposable_uninstaller --locked
cargo test -p rigpulse-apps --test native disposable_official_uninstall_verifies_success_cancellation_and_restart --locked -- --ignored
./scripts/package.ps1
./scripts/test-package.ps1
```

The native fixture tests need a Windows environment permitting their own test registrations, junctions and hard links. Sandboxed WMI/link failures were rerun successfully with approved OS access rather than weakening the protection tests. Packaging builds the normal release without screenshot or lab driver-install features. No external app was uninstalled, no user cache was cleaned, and the user's existing RigPulse installation was not replaced during validation.
