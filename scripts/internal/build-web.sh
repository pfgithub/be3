#!/usr/bin/env bash

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

wasi_sysroot=''
profile='debug'
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
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

assert_command cargo 'Install Rust from https://rustup.rs.'
assert_command clang 'Install LLVM and put its bin directory on PATH.'
assert_command llvm-ar 'Install LLVM and put its bin directory on PATH.'
cd "$repository"

tools_directory="$repository/target/tools"
output_directory="$repository/target/web"
wasi_sdk_version='33'
wasm_bindgen_version='0.2.122'
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
wasm_bindgen="$cargo_home/bin/wasm-bindgen"

if ! rustup target list --installed | grep -qx 'wasm32-wasip1'; then
    echo 'Installing the wasm32-wasip1 Rust target...'
    rustup target add wasm32-wasip1
fi

if [[ ! -x "$wasm_bindgen" ]]; then
    echo "Installing wasm-bindgen-cli $wasm_bindgen_version..."
    cargo install wasm-bindgen-cli --version "$wasm_bindgen_version"
fi
if [[ ! -x "$wasm_bindgen" ]]; then
    echo "wasm-bindgen was not installed at $wasm_bindgen" >&2
    exit 1
fi

cargo_arguments=()
if [[ "$profile" == 'release' ]]; then
    cargo_arguments+=(--release)
fi

# The bundle is the whole of what is served, so it is rebuilt from nothing
# rather than left holding what a plugin that has since gone produced.
rm -rf "$output_directory"
mkdir -p "$output_directory"

# The games are compiled for a target of their own, before the WASI toolchain
# below is exported over the environment the rest of the build runs in.
build_games "$profile"
stage_games "$output_directory/games"
write_games_index "$output_directory/games.json" 'games/'

if [[ -z "$wasi_sysroot" ]]; then
    wasi_sysroot="$tools_directory/wasi-sysroot"
    if [[ ! -d "$wasi_sysroot/include" ]]; then
        archive="$tools_directory/wasi-sysroot.tar.gz"
        url="https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-$wasi_sdk_version/wasi-sysroot-$wasi_sdk_version.0+m.tar.gz"
        echo "Downloading the WASI sysroot from $url..."
        mkdir -p "$tools_directory"
        curl --fail --location --output "$archive" "$url"
        tar -xzf "$archive" -C "$tools_directory"
        mv "$tools_directory/wasi-sysroot-$wasi_sdk_version.0+m" "$wasi_sysroot"
        rm "$archive"
    fi
fi
if [[ ! -d "$wasi_sysroot/include" ]]; then
    echo "no WASI sysroot at $wasi_sysroot" >&2
    exit 1
fi
wasi_sysroot="$(cd "$wasi_sysroot" && pwd)"
echo "Building against the WASI sysroot at $wasi_sysroot"

compiler_flags="--sysroot=$wasi_sysroot -isystem $wasi_sysroot/include/c++/v1 -mllvm -wasm-enable-sjlj -mllvm -wasm-use-legacy-eh=false -fno-exceptions -fno-rtti -DHB_NO_MT -O2 -w"
export CC_wasm32_wasip1='clang'
export CXX_wasm32_wasip1='clang++'
export AR_wasm32_wasip1='llvm-ar'
export CFLAGS_wasm32_wasip1="$compiler_flags"
export CXXFLAGS_wasm32_wasip1="$compiler_flags"
export CXXSTDLIB_wasm32_wasip1='c++'
export HARFBUZZ_SYS_NO_PKG_CONFIG='1'
export RUSTFLAGS="-C link-arg=-L$wasi_sysroot/lib/wasm32-wasip1/noeh -C link-arg=$wasi_sysroot/lib/wasm32-wasip1/libsetjmp.a"

# The app and every plugin are one cargo call, the way the native build makes
# them, so the C and Rust dependencies they share are compiled once.
load_plugins
selection=(-p block-app)
for plugin in "${plugins[@]}"; do
    selection+=(-p "$plugin")
done
echo "Building the app and ${#plugins[@]} plugins for wasm32-wasip1..."
cargo build --lib --target wasm32-wasip1 "${cargo_arguments[@]}" "${selection[@]}"

modules_directory="$repository/target/wasm32-wasip1/$profile"

# wasm-bindgen holds the whole module in memory, and a debug build of the app
# or of a plugin is hundreds of megabytes, so how many run at once is bounded
# by memory rather than by cores.
bindgen_jobs=2
bindgen_pids=()
bindgen_modules=()

await_bindings() {
    local index failed=false
    for index in "${!bindgen_pids[@]}"; do
        if ! wait "${bindgen_pids[$index]}"; then
            echo "wasm-bindgen failed for ${bindgen_modules[$index]}" >&2
            failed=true
        fi
    done
    bindgen_pids=()
    bindgen_modules=()
    if $failed; then
        exit 1
    fi
}

generate() {
    local module="$modules_directory/$1.wasm"
    if [[ ! -f "$module" ]]; then
        echo "cargo did not produce $module" >&2
        exit 1
    fi
    if [[ ${#bindgen_pids[@]} -ge $bindgen_jobs ]]; then
        await_bindings
    fi
    "$wasm_bindgen" --target web --no-typescript --out-dir "$output_directory" "$module" &
    bindgen_pids+=($!)
    bindgen_modules+=("$module")
}

echo 'Generating JavaScript bindings...'
generate block_app_lib
for plugin in "${plugins[@]}"; do
    generate "$plugin"
done
await_bindings

# A plugin runs in a worker, which has no import map, so its WASI imports are
# pointed at the shim beside it rather than at the bare specifier the app's own
# module keeps and the page's import map resolves.
for plugin in "${plugins[@]}"; do
    sed -i 's|from "wasi_snapshot_preview1"|from "./wasi.js"|g' "$output_directory/$plugin.js"
done

stage_plugin_manifests "$output_directory"
write_plugin_index "$output_directory"

cp "$internal/web/index.html" "$output_directory"
cp "$internal/web/wasi.js" "$output_directory"

size="$(du -h "$output_directory/block_app_lib_bg.wasm" | cut -f1)"
printf '\nWrote %s (%s of WebAssembly).\n' "$output_directory" "$size"
