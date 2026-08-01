param(
    [string]$AndroidSdk = $env:ANDROID_HOME
)

$ErrorActionPreference = "Stop"
$ndkVersion = "29.0.14206865"

if (-not $AndroidSdk) {
    $AndroidSdk = Join-Path $env:LOCALAPPDATA "Android\Sdk"
}

$ndk = Join-Path $AndroidSdk "ndk\$ndkVersion"
$zipalign = Join-Path $AndroidSdk "build-tools\35.0.0\zipalign.exe"
$apksigner = Join-Path $AndroidSdk "build-tools\35.0.0\apksigner.bat"
$keystore = Join-Path $PSScriptRoot "..\target\android-debug.keystore"
$apk = Join-Path $PSScriptRoot "..\target\debug\apk\block-app.apk"
$alignedApk = Join-Path $PSScriptRoot "..\target\debug\apk\block-app-aligned.apk"

foreach ($requiredPath in @($ndk, $zipalign, $apksigner, $keystore)) {
    if (-not (Test-Path -LiteralPath $requiredPath)) {
        throw "Required Android build dependency is missing: $requiredPath"
    }
}

$env:ANDROID_HOME = $AndroidSdk
$env:ANDROID_NDK_HOME = $ndk

Push-Location (Join-Path $PSScriptRoot "..\crates\block-app")
try {
    cargo apk build --lib --target aarch64-linux-android
    if ($LASTEXITCODE -ne 0) {
        throw "cargo apk build failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}

& $zipalign -P 16 -f 4 $apk $alignedApk
if ($LASTEXITCODE -ne 0) {
    throw "zipalign failed with exit code $LASTEXITCODE"
}

& $apksigner sign `
    --ks $keystore `
    --ks-pass pass:android `
    $alignedApk
if ($LASTEXITCODE -ne 0) {
    throw "apksigner failed with exit code $LASTEXITCODE"
}

Move-Item -LiteralPath $alignedApk -Destination $apk -Force
Write-Host "Built 16 KB-compatible APK: $apk"
