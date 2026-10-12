# Dot-source from the repository root when using the locally provisioned toolchain.
$rigRoot = Split-Path $PSScriptRoot -Parent
if (Test-Path "$rigRoot/.tools/cargo/bin/cargo.exe") {
    $env:CARGO_HOME = "$rigRoot/.tools/cargo"
    $env:RUSTUP_HOME = "$rigRoot/.tools/rustup"
    $env:PATH = "$env:CARGO_HOME/bin;$env:PATH"
}
$rigMsvc = Get-ChildItem "$rigRoot/.tools/msvc/VC/Tools/MSVC" -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending | Select-Object -First 1
$rigSdkRoot = 'C:/Program Files (x86)/Windows Kits/10'
$rigSdk = Get-ChildItem "$rigSdkRoot/Lib" -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending | Select-Object -First 1
if ($rigMsvc -and $rigSdk) {
    $env:PATH = "$($rigMsvc.FullName)/bin/Hostx64/x64;$rigSdkRoot/bin/$($rigSdk.Name)/x64;$env:PATH"
    $env:LIB = "$($rigMsvc.FullName)/lib/x64;$($rigSdk.FullName)/um/x64;$($rigSdk.FullName)/ucrt/x64"
    $env:INCLUDE = "$($rigMsvc.FullName)/include;$rigSdkRoot/Include/$($rigSdk.Name)/ucrt;$rigSdkRoot/Include/$($rigSdk.Name)/shared;$rigSdkRoot/Include/$($rigSdk.Name)/um"
}
