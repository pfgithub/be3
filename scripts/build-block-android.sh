#!/usr/bin/env bash

set -euo pipefail

android_sdk="${ANDROID_HOME:-}"
application_id='com.be3.block'
application_label='Block'
while [[ $# -gt 0 ]]; do
    case "$1" in
        --android-sdk)
            android_sdk="$2"
            shift 2
            ;;
        --application-id)
            application_id="$2"
            shift 2
            ;;
        --label)
            application_label="$2"
            shift 2
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

if [[ -z "$android_sdk" && "${OS:-}" == "Windows_NT" ]]; then
    android_sdk="${LOCALAPPDATA:-}/Android/Sdk"
fi
android_sdk="${android_sdk:-${ANDROID_SDK_ROOT:-}}"

if [[ -z "$android_sdk" ]]; then
    echo 'No Android SDK was found. Pass --android-sdk or set ANDROID_HOME.' >&2
    exit 1
fi

ndk_version='29.0.14206865'
build_tools_version='35.0.0'
repository="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ndk="$android_sdk/ndk/$ndk_version"
zipalign="$android_sdk/build-tools/$build_tools_version/zipalign"
apksigner="$android_sdk/build-tools/$build_tools_version/apksigner"
if [[ "${OS:-}" == "Windows_NT" ]]; then
    zipalign+='.exe'
    apksigner+='.bat'
fi
keystore="$repository/target/android-debug.keystore"
apk="$repository/target/debug/apk/block-app.apk"
aligned_apk="$repository/target/debug/apk/block-app-aligned.apk"
gradle_apk="$repository/android/app/build/outputs/apk/debug/app-debug.apk"
native_libraries="$repository/android/app/src/main/jniLibs/arm64-v8a"

for required_path in "$ndk" "$zipalign" "$apksigner" "$keystore"; do
    if [[ ! -e "$required_path" ]]; then
        echo "Required Android build dependency is missing: $required_path" >&2
        exit 1
    fi
done

export ANDROID_HOME="$android_sdk"
export ANDROID_SDK_ROOT="$android_sdk"
export ANDROID_NDK_HOME="$ndk"
export ANDROID_NDK_ROOT="$ndk"

host_tag='linux-x86_64'
if [[ "${OS:-}" == "Windows_NT" ]]; then
    host_tag='windows-x86_64'
fi
toolchain="$ndk/toolchains/llvm/prebuilt/$host_tag/bin"
cpp_runtime="$ndk/toolchains/llvm/prebuilt/$host_tag/sysroot/usr/lib/aarch64-linux-android/libc++_shared.so"
if [[ ! -f "$cpp_runtime" ]]; then
    echo "Required Android C++ runtime is missing: $cpp_runtime" >&2
    exit 1
fi
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$toolchain/aarch64-linux-android26-clang"
export CC_aarch64_linux_android="$CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"
export CXX_aarch64_linux_android="$toolchain/aarch64-linux-android26-clang++"
export AR_aarch64_linux_android="$toolchain/llvm-ar"

cargo build -p block-app --lib --target aarch64-linux-android

mkdir -p "$native_libraries" "$(dirname "$apk")"
cp "$repository/target/aarch64-linux-android/debug/libblock_app_lib.so" "$native_libraries/"
cp "$cpp_runtime" "$native_libraries/"

(
    cd "$repository/android"
    gradle --no-daemon :app:assembleDebug \
        -Pbe3ApplicationId="$application_id" \
        -Pbe3Label="$application_label"
)

"$zipalign" -P 16 -f 4 "$gradle_apk" "$aligned_apk"
"$apksigner" sign --ks "$keystore" --ks-pass pass:android "$aligned_apk"
mv -f "$aligned_apk" "$apk"
echo "Built 16 KB-compatible APK: $apk"
