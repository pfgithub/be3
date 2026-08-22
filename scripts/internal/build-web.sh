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

load_plugins
plugins_directory="$output_directory/plugins"
rm -rf "$plugins_directory"
mkdir -p "$plugins_directory"
plugin_ids=()
for plugin in "${plugins[@]}"; do
    id="$(plugin_id "$plugin")"
    echo "Building $plugin for wasm32-wasip1..."
    cargo build -p "$plugin" --target wasm32-wasip1 "${cargo_arguments[@]}"
    plugin_wasm="$repository/target/wasm32-wasip1/$profile/$plugin.wasm"
    if [[ ! -f "$plugin_wasm" ]]; then
        echo "cargo did not produce $plugin_wasm" >&2
        exit 1
    fi

    echo "Generating JavaScript bindings for $plugin..."
    plugin_directory="$plugins_directory/$id"
    mkdir -p "$plugin_directory"
    "$wasm_bindgen" --target web --no-typescript --out-dir "$plugin_directory" "$plugin_wasm"
    cp "$(plugin_manifest "$plugin")" "$plugin_directory/manifest.json"
    plugin_ids+=("$id")
done
write_plugin_index "$plugins_directory" "${plugin_ids[@]}"

echo 'Building block-app for wasm32-wasip1...'
cargo build -p block-app --lib --target wasm32-wasip1 "${cargo_arguments[@]}"
wasm="$repository/target/wasm32-wasip1/$profile/block_app_lib.wasm"
if [[ ! -f "$wasm" ]]; then
    echo "cargo did not produce $wasm" >&2
    exit 1
fi

echo 'Generating JavaScript bindings...'
mkdir -p "$output_directory"
"$wasm_bindgen" --target web --no-typescript --out-dir "$output_directory" "$wasm"
cp "$internal/web/index.html" "$output_directory"
cp "$internal/web/wasi.js" "$output_directory"

size="$(du -h "$output_directory/block_app_lib_bg.wasm" | cut -f1)"
printf '\nWrote %s (%s of WebAssembly).\n' "$output_directory" "$size"
