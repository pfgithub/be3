#!/usr/bin/env bash

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

application_id='com.be3.block'
build_arguments=("$@")
while [[ $# -gt 0 ]]; do
    case "$1" in
        --application-id)
            application_id="$2"
            shift 2
            ;;
        *)
            shift
            ;;
    esac
done

assert_command adb 'Install the Android SDK platform tools and put them on PATH.'

"$internal/build-android.sh" "${build_arguments[@]}"

apk="$repository/target/debug/apk/block-app.apk"
echo "Installing $apk..."
adb install -r "$apk"
adb shell am start -n "$application_id/com.be3.block.MainActivity"
