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
# Threads, so that the app and its plugins can move work off the thread that
# draws. The target gives the module a shared memory and wasi-libc's pthreads,
# which reach the browser through the wasi-threads import that web/threads.js
# answers by starting a Worker on the very same module and memory.
rust_target='wasm32-wasip1-threads'
wasi_sdk_version='33'
wasm_bindgen_version='0.2.122'
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
wasm_bindgen="$cargo_home/bin/wasm-bindgen"

if ! rustup target list --installed | grep -qx "$rust_target"; then
    echo "Installing the $rust_target Rust target..."
    rustup target add "$rust_target"
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

# -pthread is what marks the C objects as using atomics and bulk memory, which
# is what lets them be linked into a module with a shared memory at all.
# HarfBuzz itself is still left single-threaded, because only the thread that
# draws shapes text and HB_NO_MT keeps it from paying for locks nothing
# contends.
compiler_flags="--sysroot=$wasi_sysroot -isystem $wasi_sysroot/include/c++/v1 -pthread -mllvm -wasm-enable-sjlj -mllvm -wasm-use-legacy-eh=false -fno-exceptions -fno-rtti -DHB_NO_MT -O2 -w"
export CC_wasm32_wasip1_threads='clang'
export CXX_wasm32_wasip1_threads='clang++'
export AR_wasm32_wasip1_threads='llvm-ar'
export CFLAGS_wasm32_wasip1_threads="$compiler_flags"
export CXXFLAGS_wasm32_wasip1_threads="$compiler_flags"
export CXXSTDLIB_wasm32_wasip1_threads='c++'
export HARFBUZZ_SYS_NO_PKG_CONFIG='1'

# rustc links a cdylib with --no-entry, so lld strips the symbols wasm-bindgen
# needs to lay out a thread's own storage. Exporting them is what lets it turn
# the module into one that can be instantiated more than once.
thread_exports=''
for symbol in __heap_base __tls_base __tls_size __tls_align __wasm_init_tls; do
    thread_exports+=" -C link-arg=--export=$symbol"
done
export RUSTFLAGS="-C link-arg=-L$wasi_sysroot/lib/$rust_target/noeh -C link-arg=$wasi_sysroot/lib/$rust_target/libsetjmp.a$thread_exports"

# The app and every plugin are one cargo call, the way the native build makes
# them, so the C and Rust dependencies they share are compiled once.
load_plugins
selection=(-p block-app)
for plugin in "${plugins[@]}"; do
    selection+=(-p "$plugin")
done
echo "Building the app and ${#plugins[@]} plugins for $rust_target..."
cargo build --lib --target "$rust_target" "${cargo_arguments[@]}" "${selection[@]}"

modules_directory="$repository/target/$rust_target/$profile"

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
# pointed at the shims beside it rather than at the bare specifiers the app's
# own module keeps and the page's import map resolves. The specific shim is
# replaced first, so that what is left to match "wasi" is thread-spawn alone.
for plugin in "${plugins[@]}"; do
    sed -i 's|from "wasi_snapshot_preview1"|from "./wasi.js"|g' "$output_directory/$plugin.js"
    sed -i 's|from "wasi"|from "./threads.js"|g' "$output_directory/$plugin.js"
done

stage_plugin_manifests "$output_directory"
write_plugin_index "$output_directory"

cp "$internal/web/index.html" "$output_directory"
cp "$internal/web/wasi.js" "$output_directory"
cp "$internal/web/threads.js" "$output_directory"
cp "$internal/web/thread.js" "$output_directory"

size="$(du -h "$output_directory/block_app_lib_bg.wasm" | cut -f1)"
printf '\nWrote %s (%s of WebAssembly).\n' "$output_directory" "$size"
