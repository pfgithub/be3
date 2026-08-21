#!/usr/bin/env bash

set -euo pipefail

plugin=''
target=''
profile='debug'
output_directory=''
sign_identity=''
app_executable=''
runtime_dependencies=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --plugin)
            plugin="$2"
            shift 2
            ;;
        --target)
            target="$2"
            shift 2
            ;;
        --profile)
            profile="$2"
            shift 2
            ;;
        --output)
            output_directory="$2"
            shift 2
            ;;
        --sign-identity)
            sign_identity="$2"
            shift 2
            ;;
        --app-executable)
            app_executable="$2"
            shift 2
            ;;
        --runtime-dependency)
            runtime_dependencies+=("$2")
            shift 2
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

if [[ -z "$plugin" || -z "$target" ]]; then
    echo 'Usage: build-plugin.sh --plugin PACKAGE --target TARGET [--profile debug|release] [--output DIRECTORY] [--app-executable PATH] [--sign-identity IDENTITY] [--runtime-dependency PATH]' >&2
    exit 1
fi
if [[ "$profile" != 'debug' && "$profile" != 'release' ]]; then
    echo "Unsupported profile '$profile'; expected debug or release" >&2
    exit 1
fi
if ! command -v cargo >/dev/null; then
    echo 'cargo was not found on PATH. Install Rust from https://rustup.rs.' >&2
    exit 1
fi

repository="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -z "$output_directory" ]]; then
    output_directory="$repository/target/plugins/$plugin/$target/$profile"
fi

profile_arguments=()
if [[ "$profile" == 'release' ]]; then
    profile_arguments=(--release)
fi

cargo build -p "$plugin" --bin "$plugin-host" --target "$target" "${profile_arguments[@]}"

extension=''
case "$target" in
    *-windows-*) extension='.exe' ;;
esac
source_executable="$repository/target/$target/$profile/$plugin-host$extension"
if [[ ! -f "$source_executable" ]]; then
    echo "cargo did not produce $source_executable" >&2
    exit 1
fi

case "$target" in
    *-apple-darwin)
        destination_directory="$output_directory/Block.app/Contents/MacOS"
        app_directory="$destination_directory"
        ;;
    *-windows-*)
        destination_directory="$output_directory/$plugin"
        app_directory="$output_directory"
        ;;
    *-linux-*)
        destination_directory="$output_directory/libexec/be3"
        app_directory="$output_directory/bin"
        ;;
    *)
        echo "Target '$target' is not a supported desktop target" >&2
        exit 1
        ;;
esac

mkdir -p "$destination_directory"
cp "$source_executable" "$destination_directory/$plugin$extension"
if [[ -n "$app_executable" ]]; then
    if [[ ! -f "$app_executable" ]]; then
        echo "Application executable does not exist: $app_executable" >&2
        exit 1
    fi
    mkdir -p "$app_directory"
    cp "$app_executable" "$app_directory/block-app$extension"
fi
for dependency in "${runtime_dependencies[@]}"; do
    if [[ ! -f "$dependency" ]]; then
        echo "Runtime dependency does not exist: $dependency" >&2
        exit 1
    fi
    cp "$dependency" "$destination_directory/"
done

if [[ "$target" == *-apple-darwin && -n "$sign_identity" ]]; then
    if ! command -v codesign >/dev/null; then
        echo 'codesign was not found; macOS signing requires Xcode command-line tools' >&2
        exit 1
    fi
    codesign --force --options runtime --sign "$sign_identity" "$destination_directory/$plugin"
    if [[ -n "$app_executable" ]]; then
        codesign --force --options runtime --sign "$sign_identity" "$output_directory/Block.app"
    fi
elif [[ -n "$sign_identity" ]]; then
    echo '--sign-identity is only valid for macOS targets' >&2
    exit 1
fi

echo "Staged $plugin in $destination_directory"
