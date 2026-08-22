#!/usr/bin/env bash

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

triple=''
profile='debug'
output=''
client=true
server=true
sign_identity=''

while [[ $# -gt 0 ]]; do
    case "$1" in
        --triple)
            triple="$2"
            shift 2
            ;;
        --release)
            profile='release'
            shift
            ;;
        --output)
            output="$2"
            shift 2
            ;;
        --no-client)
            client=false
            shift
            ;;
        --no-server)
            server=false
            shift
            ;;
        --sign-identity)
            sign_identity="$2"
            shift 2
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

assert_command cargo 'Install Rust from https://rustup.rs.'
cd "$repository"

host_triple="$(rustc --version --verbose | sed -n 's/^host: //p')"
target_triple="${triple:-$host_triple}"

cargo_arguments=()
artifact_directory="$repository/target"
if [[ -n "$triple" ]]; then
    cargo_arguments+=(--target "$triple")
    artifact_directory+="/$triple"
fi
if [[ "$profile" == 'release' ]]; then
    cargo_arguments+=(--release)
fi
artifact_directory+="/$profile"

extension=''
case "$target_triple" in
    *-windows-*) extension='.exe' ;;
esac

if [[ -n "$sign_identity" ]]; then
    case "$target_triple" in
        *-apple-darwin)
            assert_command codesign 'macOS signing requires the Xcode command-line tools.'
            ;;
        *)
            echo '--sign-identity is only valid for macOS targets' >&2
            exit 1
            ;;
    esac
fi

if $server; then
    echo 'Building block-server...'
    cargo build -p block-server --bins "${cargo_arguments[@]}"
fi

plugins_directory="$artifact_directory/plugins"
games_directory="$artifact_directory/games"
if $client; then
    echo 'Building block-app...'
    cargo build -p block-app --bins "${cargo_arguments[@]}"

    load_plugins
    rm -rf "$plugins_directory"
    for plugin in "${plugins[@]}"; do
        id="$(plugin_id "$plugin")"
        echo "Building the $plugin plugin..."
        cargo build -p "$plugin" --bin "$plugin-host" "${cargo_arguments[@]}"
        executable="$artifact_directory/$plugin-host$extension"
        if [[ ! -f "$executable" ]]; then
            echo "cargo did not produce $executable" >&2
            exit 1
        fi
        plugin_directory="$plugins_directory/$id"
        mkdir -p "$plugin_directory"
        cp "$(plugin_manifest "$plugin")" "$plugin_directory/manifest.json"
        cp "$executable" "$plugin_directory/$plugin-host$extension"
        if [[ -n "$sign_identity" ]]; then
            codesign --force --options runtime --sign "$sign_identity" \
                "$plugin_directory/$plugin-host$extension"
        fi
    done

    build_games "$games_directory" "$profile"

    "$internal/fetch-pdfium.sh" --triple "$target_triple" --output "$artifact_directory"

    if [[ -n "$sign_identity" ]]; then
        codesign --force --options runtime --sign "$sign_identity" \
            "$artifact_directory/block-app$extension"
    fi
fi

if [[ -n "$output" ]]; then
    mkdir -p "$output"
    if $server; then
        cp "$artifact_directory/block-server$extension" "$output/"
    fi
    if $client; then
        cp "$artifact_directory/block-app$extension" "$output/"
        for library in libpdfium.so libpdfium.dylib pdfium.dll; do
            if [[ -f "$artifact_directory/$library" ]]; then
                cp "$artifact_directory/$library" "$output/"
            fi
        done
        rm -rf "$output/plugins"
        cp -R "$plugins_directory" "$output/plugins"
        rm -rf "$output/games"
        cp -R "$games_directory" "$output/games"
    fi
    echo "Packaged $target_triple in $output"
fi

echo "Built $target_triple in $artifact_directory"
