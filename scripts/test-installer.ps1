param([Parameter(Mandatory=$true)][string]$Msi,[string]$UpgradeMsi='')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$Msi=[IO.Path]::GetFullPath($Msi)
$installed=Join-Path $env:LOCALAPPDATA 'Programs/RigPulse/RigPulse.exe'
if(Test-Path -LiteralPath $installed){throw 'An existing RigPulse installation was found. Smoke test will not replace it.'}
New-Item -ItemType Directory -Force (Join-Path $root 'artifacts/installer') | Out-Null
$data=Join-Path $env:LOCALAPPDATA 'RigPulse'
$settings=Join-Path $data 'settings.json'
$createdSettings=$false
if(-not(Test-Path -LiteralPath $settings)){
  New-Item -ItemType Directory -Force $data | Out-Null
  [IO.File]::WriteAllText($settings,'{"power_saver":true,"include_sensitive":false}')
  $createdSettings=$true
}
$before=(Get-FileHash -LiteralPath $settings).Hash
$shortcut=Join-Path $env:APPDATA 'Microsoft/Windows/Start Menu/Programs/RigPulse/RigPulse.lnk'
$installer=New-Object -ComObject WindowsInstaller.Installer
function Get-ProductCode([string]$path){
  $database=$installer.OpenDatabase($path,0)
  $view=$database.OpenView("SELECT ``Value`` FROM ``Property`` WHERE ``Property``='ProductCode'")
  [void]$view.Execute();$record=$view.Fetch();$code=[string]$record.StringData(1);[void]$view.Close();return $code
}
$originalCode=[string](Get-ProductCode $Msi)
function Invoke-Msi([string]$mode,[string]$path,[string]$log){
  $process=Start-Process msiexec.exe -ArgumentList @($mode,"`"$path`"",'/qn','/norestart','/l*v',"`"$log`"") -WindowStyle Hidden -PassThru -Wait
  if($process.ExitCode -notin @(0,3010)){throw "MSI $mode failed with $($process.ExitCode). Log: $log"}
  Write-Output "MSI $mode exit $($process.ExitCode)"
}
$installSucceeded=$false
$activeMsi=$Msi
try {
  Invoke-Msi '/i' $Msi (Join-Path $root 'artifacts/installer/install.log')
  $installSucceeded=$true
  if(-not(Test-Path -LiteralPath $installed)){throw 'Installed executable missing'}
  if(-not(Test-Path -LiteralPath $shortcut)){throw 'Start Menu shortcut missing'}
  if(-not(Test-Path -LiteralPath (Join-Path (Split-Path $installed -Parent) 'docs/TEST_PLAN.md'))){throw 'Installed documentation missing'}
  $expected=(Get-FileHash -LiteralPath (Join-Path (Split-Path $Msi -Parent) 'RigPulse.exe')).Hash
  if((Get-FileHash -LiteralPath $installed).Hash -ne $expected){throw 'Installed executable checksum mismatch'}
  Remove-Item Env:RIGPULSE_QA_CAPTURE,Env:RIGPULSE_QA_PAGE,Env:EFRAME_SCREENSHOT_TO -ErrorAction SilentlyContinue
  $gui=Start-Process -FilePath $installed -WindowStyle Hidden -PassThru
  Start-Sleep -Seconds 8
  $gui.Refresh()
  if($gui.HasExited){throw 'Installed GUI exited unexpectedly'}
  if($gui.MainWindowTitle -ne 'RigPulse'){throw "Expected native RigPulse window, got '$($gui.MainWindowTitle)'"}
  if(-not $gui.Responding){throw 'Installed GUI is not responding'}
  if(-not $gui.CloseMainWindow()){throw 'Unable to request normal GUI close'}
  if(-not $gui.WaitForExit(15000)){throw 'GUI did not close normally'}
  Write-Output 'Installed GUI launched, responded and closed normally; shortcut and file hash verified.'
  if($UpgradeMsi){
    $UpgradeMsi=[IO.Path]::GetFullPath($UpgradeMsi)
    Invoke-Msi '/i' $UpgradeMsi (Join-Path $root 'artifacts/installer/upgrade.log')
    $activeMsi=$UpgradeMsi
    if(-not(Test-Path -LiteralPath $installed)){throw 'Upgrade lost executable'}
    if((Get-FileHash -LiteralPath $installed).Hash -ne $expected){throw 'Upgrade changed payload unexpectedly'}
    $newCode=[string](Get-ProductCode $UpgradeMsi)
    $oldState=$installer.GetType().InvokeMember('ProductState',[Reflection.BindingFlags]::GetProperty,$null,$installer,@($originalCode))
    $newState=$installer.GetType().InvokeMember('ProductState',[Reflection.BindingFlags]::GetProperty,$null,$installer,@($newCode))
    $newVersion=(Get-ItemProperty -LiteralPath 'HKCU:/Software/RigPulse').ApplicationVersion
    if($oldState -ne -1 -or $newState -ne 5 -or $newVersion -ne '0.1.1'){throw 'Major upgrade registration did not replace old product'}
    Write-Output 'Major upgrade replaced product registration with one 0.1.1 QA installation.'
  }
} finally {
  if($installSucceeded){Invoke-Msi '/x' $activeMsi (Join-Path $root 'artifacts/installer/uninstall.log')}
}
if(Test-Path -LiteralPath $installed){throw 'Uninstall left the executable behind'}
if(Test-Path -LiteralPath (Join-Path (Split-Path $installed -Parent) 'docs/TEST_PLAN.md')){throw 'Uninstall left documentation behind'}
if(Test-Path -LiteralPath $shortcut){throw 'Uninstall left the shortcut behind'}
if($before -and ((Get-FileHash -LiteralPath $settings).Hash -ne $before)){throw 'Uninstall modified existing preferences'}
Write-Output 'Uninstall removed application/shortcut and preserved existing preferences.'
if($createdSettings){Remove-Item -LiteralPath $settings}
[IO.File]::WriteAllText((Join-Path $root 'artifacts/installer/result.txt'),'PASS: install, installed-file hash, Start Menu shortcut, native GUI responding/normal close, major upgrade when supplied, uninstall, preference preservation.')
