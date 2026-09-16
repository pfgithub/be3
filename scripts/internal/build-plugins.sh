#!/usr/bin/env bash
#
# Builds every plugin and every game to WebAssembly.
#
# Neither is built for the machine the app runs on: a plugin is a WASI module
# and a game is a wasm one, and the same bytes are what Windows, macOS, Linux,
# Android and the browser all load. Building them once here is what keeps six
# native builds from each compiling the same modules again, and it is the only
# place the wasm toolchain is needed.
#
# Usage:
#   build-plugins.sh [--release] [--output DIRECTORY] [--wasi-sysroot DIRECTORY]

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

wasi_sysroot=''
profile='debug'
output=''
while [[ $# -gt 0 ]]; do
    case "$1" in
        --wasi-sysroot)
            wasi_sysroot="$2"
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
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

assert_command cargo 'Install Rust from https://rustup.rs.'
cd "$repository"

load_plugins
directory="$repository/target/plugins/$profile"
build_plugin_wasm "$profile" "$directory"
stage_plugin_manifests "$directory"

# Nothing ships a game: a module reaches the app as a game module block someone
# imports the file into. Building them is what says they still compile.
build_games "$profile"

# A manifest names its entry point and the app resolves that against the
# directory the manifest was found in, so the two travel together or neither
# does.
if [[ -n "$output" ]]; then
    mkdir -p "$output"
    rm -f "$output"/*.wasm "$output"/*.plugin.json
    cp "$directory"/*.wasm "$output/"
    for manifest in "${plugin_manifests[@]}"; do
        cp "$directory/$manifest" "$output/$manifest"
    done
    echo "Packaged ${#plugins[@]} plugins in $output"
fi

echo "Built ${#plugins[@]} plugins in $directory"
