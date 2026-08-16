#!/usr/bin/env bash

set -euo pipefail

android_sdk="${ANDROID_HOME:-}"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --android-sdk)
            android_sdk="$2"
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

(
    cd "$repository/crates/block-app"
    cargo apk build --lib --target aarch64-linux-android
)

"$zipalign" -P 16 -f 4 "$apk" "$aligned_apk"
"$apksigner" sign --ks "$keystore" --ks-pass pass:android "$aligned_apk"
mv -f "$aligned_apk" "$apk"
echo "Built 16 KB-compatible APK: $apk"
