param(
    [string]$AndroidSdk = $env:ANDROID_HOME
)

$ErrorActionPreference = "Stop"
$ndkVersion = "29.0.14206865"
$buildToolsVersion = "35.0.0"
$onWindows = $env:OS -eq "Windows_NT"

if (-not $AndroidSdk) {
    $AndroidSdk = if ($onWindows) {
        Join-Path $env:LOCALAPPDATA "Android/Sdk"
    } else {
        $env:ANDROID_SDK_ROOT
    }
}

if (-not $AndroidSdk) {
    throw "No Android SDK was found. Pass -AndroidSdk or set ANDROID_HOME."
}

# The build tools ship Windows wrappers alongside the Unix executables.
$zipalignName = if ($onWindows) { "zipalign.exe" } else { "zipalign" }
$apksignerName = if ($onWindows) { "apksigner.bat" } else { "apksigner" }

$repository = Split-Path -Parent $PSScriptRoot
$ndk = Join-Path $AndroidSdk "ndk/$ndkVersion"
$zipalign = Join-Path $AndroidSdk "build-tools/$buildToolsVersion/$zipalignName"
$apksigner = Join-Path $AndroidSdk "build-tools/$buildToolsVersion/$apksignerName"
$keystore = Join-Path $repository "target/android-debug.keystore"
$apk = Join-Path $repository "target/debug/apk/block-app.apk"
$alignedApk = Join-Path $repository "target/debug/apk/block-app-aligned.apk"

foreach ($requiredPath in @($ndk, $zipalign, $apksigner, $keystore)) {
    if (-not (Test-Path -LiteralPath $requiredPath)) {
        throw "Required Android build dependency is missing: $requiredPath"
    }
}

$env:ANDROID_HOME = $AndroidSdk
$env:ANDROID_SDK_ROOT = $AndroidSdk
$env:ANDROID_NDK_HOME = $ndk
$env:ANDROID_NDK_ROOT = $ndk

Push-Location (Join-Path $repository "crates/block-app")
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
