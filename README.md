# BE3

## Building Block for Android

`block-app` builds as an ARM64 Android APK. The embedded browser and image
clipboard integrations are unavailable on Android.

### Prerequisites

- Rust and Cargo
- Java Development Kit 17 or newer
- Android SDK command-line tools
- Android SDK Platform 35, Build-Tools 35.0.0, Platform-Tools, and NDK
  27.3.13750724
- `cargo-apk` 0.10.0

Install the Rust target and APK packaging tool:

```powershell
rustup target add aarch64-linux-android
cargo install cargo-apk --version 0.10.0
```

Use Android's SDK Manager to accept the licenses and install the required
packages:

```powershell
sdkmanager --licenses
sdkmanager "platform-tools" "platforms;android-35" "build-tools;35.0.0" "ndk;27.3.13750724"
```

Set the SDK paths for the current PowerShell session. Replace the SDK path if
your installation is elsewhere:

```powershell
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:ANDROID_NDK_HOME = "$env:ANDROID_HOME\ndk\27.3.13750724"
```

### Build the APK

The development build uses an ignored debug keystore at
`target\android-debug.keystore`. Create it once from the repository root:

```powershell
keytool -genkeypair -v `
  -keystore target\android-debug.keystore `
  -storepass android `
  -alias androiddebugkey `
  -keypass android `
  -dname "CN=Android Debug,O=Android,C=US" `
  -keyalg RSA `
  -keysize 2048 `
  -validity 10000
```

Skip that command when the keystore already exists. Build the APK from the
repository root:

```powershell
cd crates\block-app
cargo apk build --lib --target aarch64-linux-android
cd ..\..
```

The signed development APK is written to:

```text
target\debug\apk\block-app.apk
```

### Install and run over USB

Enable Developer options and USB debugging on the Android phone, connect it by
USB, unlock it, and accept the debugging authorization prompt. Verify that ADB
can see it:

```powershell
& "$env:ANDROID_HOME\platform-tools\adb.exe" devices
```

The device status must be `device`. Install or update the app, then launch it:

```powershell
& "$env:ANDROID_HOME\platform-tools\adb.exe" install -r target\debug\apk\block-app.apk
& "$env:ANDROID_HOME\platform-tools\adb.exe" shell monkey -p com.be3.block -c android.intent.category.LAUNCHER 1
```

If ADB reports `unauthorized`, reconnect the phone and accept its USB debugging
prompt. If multiple devices are connected, pass `-s SERIAL` immediately after
`adb.exe`, using a serial listed by `adb devices`.
