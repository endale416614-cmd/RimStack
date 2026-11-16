$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..\..")

$metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
$version = ($metadata.packages | Where-Object name -eq "rimstack-app").version
$cargoProfile = if ($env:RIMSTACK_PROFILE) { $env:RIMSTACK_PROFILE } else { "dist" }

if (-not ((rustc -vV) -match "^host: x86_64-pc-windows-msvc$")) {
    Write-Error "Build with the x86_64-pc-windows-msvc toolchain: the DDS encoder ships for MSVC alone"
}
cargo build --profile $cargoProfile -p rimstack-app
if ($LASTEXITCODE) { exit $LASTEXITCODE }

$stage = "target\packaging\RimStack"
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory $stage | Out-Null
Copy-Item "target\$cargoProfile\rimstack.exe" $stage

# The C++ runtime beside the program: the DDS encoder's prebuilt MSVC libraries link it dynamically.
$crt = Get-ChildItem (Join-Path $env:VCToolsRedistDir "x64") -Directory -Filter "Microsoft.VC*.CRT" | Select-Object -First 1
if (-not $crt) { Write-Error "No C++ runtime under VCToolsRedistDir; run from a Developer prompt" }
foreach ($dll in "msvcp140.dll", "vcruntime140.dll", "vcruntime140_1.dll") {
    Copy-Item (Join-Path $crt.FullName $dll) $stage
}

function Sign($file) {
    if (-not $env:SIGN_CERT) { return }
    signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /f $env:SIGN_CERT /p $env:SIGN_PASSWORD $file
    if ($LASTEXITCODE) { exit $LASTEXITCODE }
}
Sign "$stage\rimstack.exe"

New-Item -ItemType Directory target\packages -Force | Out-Null
$zip = "target\packages\RimStack-$version-windows-x86_64.zip"
Compress-Archive -Path "$stage\*" -DestinationPath $zip -Force
Write-Output $zip

if (Get-Command ISCC -ErrorAction SilentlyContinue) {
    ISCC /Qp "/DVersion=$version" "/DStage=$((Resolve-Path $stage).Path)" "/Otarget\packages" packaging\windows\rimstack.iss
    if ($LASTEXITCODE) { exit $LASTEXITCODE }
    $setup = "target\packages\RimStack-$version-setup.exe"
    Sign $setup
    Write-Output $setup
}
