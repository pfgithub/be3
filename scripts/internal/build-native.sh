#!/usr/bin/env bash

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

triple=''
profile='debug'
output=''
client=true
server=true
# The plugins and games are wasm, identical whatever the app is built for, so a
# build that is one of several for different machines leaves them to
# build-plugins.sh rather than compiling the same modules again.
with_plugins=true
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
        --no-plugins)
            with_plugins=false
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

sign() {
    if [[ -n "$sign_identity" ]]; then
        codesign --force --options runtime --sign "$sign_identity" "$1"
    fi
}

# The app, the server and the compiler the plugins are handed to are one cargo
# call, so cargo builds the dependencies they share once and has all three to
# spread across the cores rather than a stretch of each in turn. Every plugin is
# a wasm guest, built for its own target below.
load_plugins
selection=()
building=()
if $client; then
    selection+=(-p block-app --bin block-app)
    building+=('the app')
fi
if $server; then
    selection+=(-p block-server --bin block-server)
    building+=('the server')
fi
if [[ ${#selection[@]} -eq 0 ]]; then
    echo '--no-client and --no-server together leave nothing to build' >&2
    exit 1
fi
# The compiler runs here rather than on the machine the app is for, so it only
# joins this call when the two are the same machine. A cross build compiles it
# on its own afterwards, for this machine and with every backend.
compiler=false
if $client && [[ -z "$triple" ]]; then
    compiler=true
    selection+=(-p block-wasm-host --bin precompile)
    building+=('the plugin compiler')
fi
# Negative array subscripts need bash 4.3+, which macOS does not ship: its
# /bin/bash is stuck on the last GPLv2 release, 3.2.
last_index=$((${#building[@]} - 1))
last="${building[$last_index]}"
unset "building[$last_index]"
description="$last"
if [[ ${#building[@]} -ne 0 ]]; then
    description="$(printf '%s, ' "${building[@]}")"
    description="${description%, } and $last"
fi
# The terminal emulator the debug terminal window is built on is Zig, and is
# compiled and cached for the target before cargo links it in.
if $client; then
    "$internal/build-ghostty-vt.sh" --triple "$target_triple" > /dev/null
fi

echo "Building $description..."
cargo build "${cargo_arguments[@]}" "${selection[@]}"
if $compiler; then
    use_precompiler "$artifact_directory"
fi

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

if $client && $with_plugins; then
    build_plugin_wasm "$profile" "$artifact_directory"
    precompile_plugin_wasm "$artifact_directory" "$target_triple"
    stage_plugin_manifests "$artifact_directory"
    build_games "$profile"
fi

# PDFium is a native library the PDF plugin loads at runtime, so it belongs to
# the machine this is built for however the modules are built.
if $client; then
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
        if $with_plugins; then
            rm -f "$output"/*.plugin.json
            for manifest in "${plugin_manifests[@]}"; do
                cp "$artifact_directory/$manifest" "$output/$manifest"
            done
        fi
    fi
    echo "Packaged $target_triple in $output"
fi

echo "Built $target_triple in $artifact_directory"
