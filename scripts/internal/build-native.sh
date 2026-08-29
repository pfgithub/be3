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
games_prefix='../wasm32-unknown-unknown'
if [[ -n "$triple" ]]; then
    cargo_arguments+=(--target "$triple")
    artifact_directory+="/$triple"
    games_prefix="../$games_prefix"
fi
if [[ "$profile" == 'release' ]]; then
    cargo_arguments+=(--release)
fi
artifact_directory+="/$profile"
games_prefix+="/$profile/"

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

sign() {
    if [[ -n "$sign_identity" ]]; then
        codesign --force --options runtime --sign "$sign_identity" "$1"
    fi
}

# The app and the server are one cargo call, so cargo builds the dependencies
# they share once. Every plugin is a wasm guest, built for its own target below.
load_plugins
selection=()
if $server; then
    selection+=(-p block-server --bin block-server)
fi
if $client; then
    selection+=(-p block-app --bin block-app)
fi
if [[ ${#selection[@]} -eq 0 ]]; then
    echo '--no-client and --no-server together leave nothing to build' >&2
    exit 1
fi
echo 'Building the app and the server...'
cargo build "${cargo_arguments[@]}" "${selection[@]}"

executables=()
if $server; then
    executables+=("block-server$extension")
fi
if $client; then
    executables+=("block-app$extension")
fi
for executable in "${executables[@]}"; do
    if [[ ! -f "$artifact_directory/$executable" ]]; then
        echo "cargo did not produce $artifact_directory/$executable" >&2
        exit 1
    fi
    sign "$artifact_directory/$executable"
done

if $client; then
    build_plugin_wasm "$profile" "$artifact_directory"
    stage_plugin_manifests "$artifact_directory"
    build_games "$profile"
    write_games_index "$artifact_directory/games.json" "$games_prefix"
    "$internal/fetch-pdfium.sh" --triple "$target_triple" --output "$artifact_directory"
fi

# Packaging is the one place files are copied: what a directory to hand over
# holds cannot be spread across the target directory the way a local build is.
if [[ -n "$output" ]]; then
    mkdir -p "$output"
    for executable in "${executables[@]}"; do
        cp "$artifact_directory/$executable" "$output/$executable"
    done
    if $client; then
        for library in libpdfium.so libpdfium.dylib pdfium.dll; do
            if [[ -f "$artifact_directory/$library" ]]; then
                cp "$artifact_directory/$library" "$output/"
            fi
        done
        rm -f "$output"/*.plugin.json
        for manifest in "${plugin_manifests[@]}"; do
            cp "$artifact_directory/$manifest" "$output/$manifest"
        done
        stage_games "$output/games"
        write_games_index "$output/games.json" 'games/'
    fi
    echo "Packaged $target_triple in $output"
fi

echo "Built $target_triple in $artifact_directory"
