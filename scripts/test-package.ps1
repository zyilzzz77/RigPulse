param([string]$Msi='')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$version=(Select-String -Path (Join-Path $root 'Cargo.toml') -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
if(-not $Msi){$Msi=Join-Path $root "dist/RigPulse-$version-x64.msi"}
$Msi=[IO.Path]::GetFullPath($Msi)
$work=Join-Path $root "artifacts/package-test-$version"
New-Item -ItemType Directory -Force $work | Out-Null
$target=Join-Path $work ('extract-'+[guid]::NewGuid().ToString('N'))
$settings=Join-Path $env:LOCALAPPDATA 'RigPulse/settings.json'
$before=if(Test-Path -LiteralPath $settings){(Get-FileHash -LiteralPath $settings).Hash}else{''}
# Administrative image extraction does not install/upgrade the user's existing application.
$arguments=@('/a',"`"$Msi`"",'/qn','/norestart',"TARGETDIR=`"$target`"",'/l*v',"`"$work/extract.log`"")
$process=Start-Process msiexec.exe -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru
if($process.ExitCode -ne 0){throw "MSI extraction failed: $($process.ExitCode)"}
$exe=Get-ChildItem -LiteralPath $target -Filter RigPulse.exe -Recurse | Select-Object -First 1
if(-not $exe){throw 'MSI payload did not contain RigPulse.exe'}
if((Get-FileHash -LiteralPath $exe.FullName).Hash -ne (Get-FileHash -LiteralPath (Join-Path $root 'dist/RigPulse.exe')).Hash){throw 'MSI executable hash mismatch'}
foreach($doc in @('UI_UX_OVERHAUL.md','APPLICATION_UPDATES.md','APPS_MANAGER.md','UPGRADE_PHASE1.md','COMPLETE_UPGRADE.md','TEST_PLAN.md')){
  if(-not(Test-Path -LiteralPath (Join-Path $exe.DirectoryName "docs/$doc"))){throw "MSI documentation missing: $doc"}
  if((Get-FileHash -LiteralPath (Join-Path $exe.DirectoryName "docs/$doc")).Hash -ne (Get-FileHash -LiteralPath (Join-Path $root "docs/$doc")).Hash){throw "MSI documentation hash mismatch: $doc"}
}
$fileVersion=[Diagnostics.FileVersionInfo]::GetVersionInfo($exe.FullName)
if($fileVersion.ProductVersion -notlike "$version*"){throw 'Executable version mismatch'}
Remove-Item Env:RIGPULSE_QA_CAPTURE,Env:RIGPULSE_QA_PAGE,Env:RIGPULSE_QA_SIZE,Env:RIGPULSE_QA_SCROLL,Env:EFRAME_SCREENSHOT_TO -ErrorAction SilentlyContinue
$gui=Start-Process -FilePath $exe.FullName -WindowStyle Hidden -PassThru
try{
  Start-Sleep -Seconds 10
  $gui.Refresh()
  if($gui.HasExited -or -not $gui.Responding -or $gui.MainWindowTitle -ne 'RigPulse'){throw 'Release GUI failed smoke test'}
}finally{
  if(-not $gui.HasExited){[void]$gui.CloseMainWindow();if(-not $gui.WaitForExit(15000)){throw 'Release GUI did not close normally'}}
}
if($before -and ((Get-FileHash -LiteralPath $settings).Hash -ne $before)){throw 'Smoke test unexpectedly changed user preferences'}
$result="MSI $version extracted; payload/hash/docs/version verified; production GUI launched, responded and closed normally; user preferences preserved. Existing installation was not upgraded/uninstalled."
$result | Set-Content -LiteralPath (Join-Path $work 'result.txt')
Write-Output $result

