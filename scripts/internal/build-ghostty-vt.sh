#!/usr/bin/env bash

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

# The Ghostty commit libghostty-vt is built from. Bump this to pick up a newer
# terminal emulator; the checkout under external/ is refreshed automatically
# when the commit here no longer matches what is in it.
ghostty_repository='https://github.com/ghostty-org/ghostty.git'
ghostty_commit='a887df42c56f6de86c0fe6da9c4eeca37931e083'

triple=''
while [[ $# -gt 0 ]]; do
    case "$1" in
        --triple)
            triple="$2"
            shift 2
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

if [[ -z "$triple" ]]; then
    echo 'Usage: build-ghostty-vt.sh --triple TRIPLE' >&2
    exit 1
fi

# libghostty-vt is Zig, and Zig is a cross compiler for every target the app is
# built for, so the same toolchain produces the archive for all of them.
zig_cpu=''
zig_optimize='ReleaseFast'
case "$triple" in
    # The library is only input and output, so the browser gets it as a
    # freestanding archive linked into the app's own module. Freestanding is
    # what also turns off the Kitty graphics protocol, whose image loading
    # wants a filesystem and a clock that WebAssembly has neither of.
    wasm32-*)
        zig_target='wasm32-freestanding'
        # What the app's own objects are compiled with: an object without
        # these cannot be linked into a module with a shared memory at all.
        zig_cpu='generic+atomics+bulk_memory+mutable_globals+sign_ext'
        zig_optimize='ReleaseSmall'
        ;;
    x86_64-unknown-linux-gnu) zig_target='x86_64-linux-gnu' ;;
    aarch64-unknown-linux-gnu) zig_target='aarch64-linux-gnu' ;;
    x86_64-unknown-linux-musl) zig_target='x86_64-linux-musl' ;;
    aarch64-unknown-linux-musl) zig_target='aarch64-linux-musl' ;;
    x86_64-apple-darwin) zig_target='x86_64-macos-none' ;;
    aarch64-apple-darwin) zig_target='aarch64-macos-none' ;;
    aarch64-linux-android) zig_target='aarch64-linux-android' ;;
    x86_64-linux-android) zig_target='x86_64-linux-android' ;;
    armv7-linux-androideabi) zig_target='arm-linux-androideabi' ;;
    x86_64-pc-windows-msvc) zig_target='x86_64-windows-msvc' ;;
    aarch64-pc-windows-msvc) zig_target='aarch64-windows-msvc' ;;
    x86_64-pc-windows-gnu) zig_target='x86_64-windows-gnu' ;;
    aarch64-pc-windows-gnullvm) zig_target='aarch64-windows-gnu' ;;
    *)
        echo "No libghostty-vt build is known for target $triple" >&2
        exit 1
        ;;
esac

output="$repository/target/ghostty-vt/$triple"
# Zig names the archive after the target's own convention, and the MSVC linker
# wants that name; the GNU toolchains want the lib prefix that -l resolves.
built_name='libghostty-vt.a'
archive_name='libghostty-vt.a'
case "$triple" in
    *-windows-msvc)
        built_name='ghostty-vt-static.lib'
        archive_name='ghostty-vt-static.lib'
        ;;
    *-windows-*) built_name='ghostty-vt-static.lib' ;;
esac
archive="$output/$archive_name"

# Everything that decides what ends up in the archive, so that changing any of
# it rebuilds rather than leaving a stale artifact behind.
stamp="$output/.stamp"
stamp_contents="$ghostty_commit $zig_target $zig_cpu $zig_optimize"
if [[ -f "$archive" && -f "$stamp" && "$(cat "$stamp")" == "$stamp_contents" ]]; then
    echo "$archive"
    exit 0
fi

assert_command git 'Install git.'
assert_command zig 'Install Zig 0.15.2 or newer from https://ziglang.org/download.'

# The checkout is deliberately outside the target directory: it is well over a
# hundred megabytes and survives a cargo clean, and only ever holds the one
# commit that is fetched into it.
source_directory="$repository/external/ghostty"
if [[ "$(cat "$source_directory/.commit" 2> /dev/null)" != "$ghostty_commit" ]]; then
    echo "Fetching Ghostty $ghostty_commit..." >&2
    rm -rf "$source_directory"
    mkdir -p "$source_directory"
    git -C "$source_directory" init --quiet
    git -C "$source_directory" remote add origin "$ghostty_repository"
    git -C "$source_directory" fetch --quiet --depth 1 origin "$ghostty_commit"
    git -C "$source_directory" checkout --quiet FETCH_HEAD
    echo "$ghostty_commit" > "$source_directory/.commit"
fi

install_prefix="$output/install"
rm -rf "$install_prefix" "$archive" "$stamp"
mkdir -p "$output"

# app-runtime=none and emit-lib-vt leave the terminal emulator and nothing
# else: no GUI, no font stack, no xcframework. SIMD is what would pull in the
# vendored C++ dependencies and a libc, which is more than a debug window
# needs and more than the browser can link.
zig_arguments=(
    build
    -Demit-lib-vt=true
    -Demit-xcframework=false
    -Dapp-runtime=none
    -Dsimd=false
    "-Doptimize=$zig_optimize"
    "-Dtarget=$zig_target"
)
if [[ -n "$zig_cpu" ]]; then
    zig_arguments+=("-Dcpu=$zig_cpu")
fi

echo "Building libghostty-vt for $triple ($zig_target)..." >&2
(
    cd "$source_directory"
    zig "${zig_arguments[@]}" \
        --prefix "$install_prefix" \
        --cache-dir "$repository/target/ghostty-vt/zig-cache"
)

built="$install_prefix/lib/$built_name"
if [[ ! -f "$built" ]]; then
    echo "zig did not produce $built" >&2
    exit 1
fi
mv "$built" "$archive"
rm -rf "$install_prefix"
echo "$stamp_contents" > "$stamp"
echo "$archive"
