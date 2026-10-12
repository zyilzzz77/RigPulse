param([string]$WixBin='', [switch]$SkipBuild)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Set-Location $root
. "$PSScriptRoot/dev-env.ps1"
if(-not $WixBin){$WixBin=Join-Path $root '.tools/wix'}
$WixBin=[IO.Path]::GetFullPath($WixBin)
if(-not(Test-Path "$WixBin/candle.exe")){throw 'WiX 3.14.1 required. Set -WixBin to its portable binary folder.'}
if(-not $SkipBuild){cargo build --release -p rigpulse --locked; if($LASTEXITCODE -ne 0){throw 'Release build failed'}}
$version=(Select-String -Path Cargo.toml -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
$stage=Join-Path $root 'dist/staging'
New-Item -ItemType Directory -Force $stage,artifacts/package | Out-Null
Copy-Item -LiteralPath target/release/RigPulse.exe -Destination "$stage/RigPulse.exe"
Copy-Item -LiteralPath README.md -Destination "$stage/README.md"
Copy-Item -LiteralPath LICENSE -Destination "$stage/LICENSE.txt"
New-Item -ItemType Directory -Force "$stage/docs" | Out-Null
foreach($document in @('ARCHITECTURE.md','LIMITATIONS.md','PHASE_LOG.md','TEST_PLAN.md','UI_UX_OVERHAUL.md','APPLICATION_UPDATES.md','APPS_MANAGER.md','UPGRADE_PHASE1.md','COMPLETE_UPGRADE.md')){Copy-Item -LiteralPath "docs/$document" -Destination "$stage/docs/$document"}
$metadata=cargo metadata --locked --offline --filter-platform x86_64-pc-windows-msvc --format-version 1 | ConvertFrom-Json
if($LASTEXITCODE -ne 0){throw 'Dependency inventory failed'}
$notices=[Collections.Generic.List[string]]::new()
$notices.Add('RigPulse third-party dependency notices. Package versions are pinned in Cargo.lock.')
$notices.Add('NVIDIA NVML is an optional system driver library; its binary is not redistributed. Windows fonts are loaded from the operating system and not bundled. Ookla Speedtest CLI is proprietary and is NOT bundled in the MSI/ZIP; optional first-use download is from install.speedtest.net after user agreement. Its publisher documentation and license links accompany the separate download.')
foreach($p in ($metadata.packages | Sort-Object name,version)){
  $notices.Add("`r`n===== $($p.name) $($p.version) | $($p.license) | $($p.repository) =====")
  $folder=Split-Path $p.manifest_path -Parent
  foreach($file in (Get-ChildItem -LiteralPath $folder -File | Where-Object {$_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE)([.-]|$)'})){
    $notices.Add("--- $($file.Name) ---")
    $notices.Add([IO.File]::ReadAllText($file.FullName))
  }
}
[IO.File]::WriteAllLines("$stage/THIRD_PARTY_NOTICES.txt",$notices)
$msi=Join-Path $root "dist/RigPulse-$version-x64.msi"
& "$WixBin/candle.exe" -nologo -arch x64 "-dVersion=$version" "-dSourceDir=$stage" "-dLicenseRtf=$root/wix/license.rtf" wix/main.wxs -out artifacts/package/RigPulse.wixobj
if($LASTEXITCODE -ne 0){throw 'WiX compilation failed'}
& "$WixBin/light.exe" -nologo -ext WixUIExtension -pdbout artifacts/package/RigPulse.wixpdb -out $msi artifacts/package/RigPulse.wixobj
if($LASTEXITCODE -ne 0){throw 'MSI linking/validation failed'}
Copy-Item -LiteralPath "$stage/RigPulse.exe" -Destination dist/RigPulse.exe
$zip=Join-Path $root "dist/RigPulse-$version-x64-portable.zip"
Compress-Archive -Path "$stage/*" -DestinationPath $zip -Force
$hashes=@($msi,$zip,(Join-Path $root 'dist/RigPulse.exe'))|ForEach-Object {$hash=Get-FileHash -LiteralPath $_ -Algorithm SHA256; "$($hash.Hash.ToLower())  $([IO.Path]::GetFileName($_))"}
[IO.File]::WriteAllLines((Join-Path $root 'dist/SHA256SUMS.txt'),$hashes)
Write-Output "Packaged $msi"

