# BE3

## Continuous integration

`.github/workflows/ci.yml` runs on every push to `main` and on every pull
request. It checks formatting with `cargo fmt --all --check`, runs the test
suite with `cargo nextest run --workspace --profile ci`, and builds the web
bundle, the Android APK, and native client and server binaries. Every build
uploads its output as a workflow artifact:

| Artifact | Contents |
| --- | --- |
| `block-web` | the `target/web` WebAssembly bundle |
| `block-app-android-aarch64` | the signed development APK |
| `block-linux-x86_64` | `block-app` and `block-server` |
| `block-linux-aarch64` | `block-server` |
| `block-windows-x86_64` | `block-app.exe` and `block-server.exe` |
| `block-windows-aarch64` | `block-app.exe` and `block-server.exe` |
| `block-macos-aarch64` | `block-app` and `block-server` |
| `block-macos-x86_64` | `block-app` and `block-server` |

Each operating system builds on its own runner, because the client links that
platform's window system, webview, and clipboard libraries. Within a runner the
second architecture is cross-compiled: macOS builds x86\_64 from the ARM64
runner and Windows builds ARM64 from the x86\_64 runner, since both toolchains
ship every architecture's headers and libraries. Linux is the exception. Only
`block-server` is cross-compiled to ARM64 there, with `gcc-aarch64-linux-gnu`
for the SQLite that `rusqlite` bundles; the client would additionally need GTK
and WebKitGTK built for ARM64, which Ubuntu only supplies through a multiarch
sysroot.

## Building Block for the web

`block-app` builds as a WebAssembly bundle that runs in a browser. The embedded
browser, the image clipboard, the canvas clipboard, and the native file picker
are unavailable there, map tiles are not downloaded, and there is no embedded
server, so the web build signs in to a remote server only.

The build targets `wasm32-wasip1` rather than `wasm32-unknown-unknown`. FreeType
and HarfBuzz are C and C++, so the text editor's glyph pipeline needs a libc and
a libc++ to compile against, and `wasm32-unknown-unknown` has neither.
wasm-bindgen understands WASI: it emits the `wasi_snapshot_preview1` imports and
wires them into the module's import object, which `index.html` resolves to
`wasi.js` with an import map.

### Prerequisites

- Rust and Cargo
- LLVM 17 or newer, with `clang` and `llvm-ar` on `PATH`

The build script installs the `wasm32-wasip1` Rust target and `wasm-bindgen-cli`
if they are missing, and downloads the WASI sysroot into `target/tools` on first
use. Pass `-WasiSysroot` to build against one that is already installed.

### Build

```powershell
.\scripts\build-block-web.ps1 -Release
```

The bundle is written to `target\web`. Serve it over HTTP — opening
`index.html` from disk will not work, because the browser refuses to load
WebAssembly modules over `file://`:

```powershell
python -m http.server --directory target/web 8080
```

### Signing in

The web build has no embedded server, so the account dialog asks for a remote
server URL. That server must be reachable from the page: `block-server` answers
the CORS preflight that a browser sends before every management command, but a
page served over HTTPS cannot talk to a server over plain HTTP.

## Building Block for Android

`block-app` builds as an ARM64 Android APK. The embedded browser and image
clipboard integrations are unavailable on Android.

### Prerequisites

- Rust and Cargo
- Java Development Kit 17 or newer
- Android SDK command-line tools
- Android SDK Platform 35, Build-Tools 35.0.0, Platform-Tools, and NDK
  29.0.14206865
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
sdkmanager "platform-tools" "platforms;android-35" "build-tools;35.0.0" "ndk;29.0.14206865"
```

Set the SDK paths for the current PowerShell session. Replace the SDK path if
your installation is elsewhere:

```powershell
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:ANDROID_NDK_HOME = "$env:ANDROID_HOME\ndk\29.0.14206865"
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

Skip that command when the keystore already exists. Build and 16 KB-align the
APK from the repository root:

```powershell
.\scripts\build-block-android.ps1 -AndroidSdk '/absolute/path/to/target/android-sdk'
```

The build script links the native library with 16 KB ELF page alignment, uses
NDK r29's compatible C++ runtime, and applies 16 KB APK ZIP alignment before
signing the result.

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

Logs

```
.\target\android-sdk\platform-tools\adb.exe logcat -v threadtime "AndroidRuntime:E" "libc:F" "DEBUG:F" "RustStdoutStderr:V" "*:S"
```