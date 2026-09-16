repository="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
internal="$repository/scripts/internal"

assert_command() {
    if ! command -v "$1" > /dev/null; then
        echo "$1 was not found on PATH. $2" >&2
        exit 1
    fi
}

# clang and rust-lld are native programs even when the build runs under a POSIX
# shell on Windows, and neither can open the /c/... form such a shell hands out,
# so every path that travels to one of them as an argument goes through here.
native_path() {
    if [[ "${OS:-}" == 'Windows_NT' ]] && command -v cygpath > /dev/null; then
        cygpath -m "$1"
    else
        echo "$1"
    fi
}

manifest_field() {
    sed -n "s/.*\"$2\"[[:space:]]*:[[:space:]]*\"\([^\"]*\)\".*/\1/p" "$1" | head -1
}

load_plugins() {
    plugins=()
    local manifest
    for manifest in "$repository"/crates/editors/*/manifest.json; do
        [[ -f "$manifest" ]] || continue
        plugins+=("$(basename "$(dirname "$manifest")")")
    done
    if [[ ${#plugins[@]} -eq 0 ]]; then
        echo 'No plugin manifests were found under crates/editors' >&2
        exit 1
    fi
}

plugin_manifest() {
    echo "$repository/crates/editors/$1/manifest.json"
}

plugin_id() {
    local manifest id
    manifest="$(plugin_manifest "$1")"
    if [[ ! -f "$manifest" ]]; then
        echo "No manifest at $manifest" >&2
        return 1
    fi
    id="$(manifest_field "$manifest" id)"
    if [[ -z "$id" ]]; then
        echo "$manifest has no plugin id" >&2
        return 1
    fi
    echo "$id"
}

# Puts every plugin's manifest beside the artifacts cargo produced, named after
# the plugin's id, and leaves the names it wrote in plugin_manifests. Nothing a
# plugin was compiled to is moved: the manifest names its entry point and the
# app resolves that against the directory the manifest itself was found in.
stage_plugin_manifests() {
    local directory="$1"
    mkdir -p "$directory"
    rm -f "$directory"/*.plugin.json
    plugin_manifests=()
    local plugin id
    for plugin in "${plugins[@]}"; do
        id="$(plugin_id "$plugin")"
        cp "$(plugin_manifest "$plugin")" "$directory/$id.plugin.json"
        plugin_manifests+=("$id.plugin.json")
    done
}

# The browser and Android read an index because neither can list a directory.
# Native discovery scans for the manifests instead, so it needs no index.
write_plugin_index() {
    write_index "$1/plugins.json" "${plugin_manifests[@]}"
}

write_index() {
    local file="$1"
    shift
    local entries=("$@")
    local index separator
    {
        echo '['
        for index in "${!entries[@]}"; do
            separator=','
            if [[ $index -eq $((${#entries[@]} - 1)) ]]; then
                separator=''
            fi
            echo "  \"${entries[$index]}\"$separator"
        done
        echo ']'
    } > "$file"
}

load_games() {
    games=()
    local manifest
    for manifest in "$repository"/crates/tabletop_games/rules/*/Cargo.toml; do
        [[ -f "$manifest" ]] || continue
        games+=("$(basename "$(dirname "$manifest")")")
    done
    if [[ ${#games[@]} -eq 0 ]]; then
        echo 'No games were found under crates/tabletop_games/rules' >&2
        exit 1
    fi
}

games_directory=''

# Compiles every game to its own WebAssembly module in one cargo call and
# leaves them where cargo put them, in games_directory. Nothing ships them: a
# module reaches the app as a game module block the user imports the file into,
# so the build only has to produce a file there is something to import.
build_games() {
    local profile="$1"
    local arguments=(--lib --target wasm32-unknown-unknown)
    if [[ "$profile" == 'release' ]]; then
        arguments+=(--release)
    fi
    if ! rustup target list --installed | grep -qx 'wasm32-unknown-unknown'; then
        echo 'Installing the wasm32-unknown-unknown Rust target...'
        rustup target add wasm32-unknown-unknown
    fi

    load_games
    local game
    for game in "${games[@]}"; do
        arguments+=(-p "$game")
    done
    echo "Building ${#games[@]} games..."
    (
        cd "$repository"
        unwrap_rustc_for_wasm
        cargo build "${arguments[@]}"
    )

    games_directory="$repository/target/wasm32-unknown-unknown/$profile"
    for game in "${games[@]}"; do
        if [[ ! -f "$games_directory/$game.wasm" ]]; then
            echo "cargo did not produce $games_directory/$game.wasm" >&2
            exit 1
        fi
    done
}

# Both the WASI toolchain and the guest flags a plugin needs to compile without
# wasm-bindgen. The web build exports the same environment for its own cargo
# call; a plugin built for wasmtime is the same target with none of the glue.
wasi_sdk_version='33'
wasm_rust_target='wasm32-wasip1-threads'

# The flags below are LLVM 19's, so an older clang stops at the first of them
# with nothing built. Ubuntu still ships 18 as plain clang while packaging newer
# ones beside it, so a machine that has one is used rather than turned away.
wasm_clang_minimum='19'
wasm_clang=''
wasm_clangxx=''
wasm_ar=''

clang_major() {
    command -v "$1" > /dev/null || return 1
    "$1" --version 2> /dev/null | sed -n 's/.*clang version \([0-9][0-9]*\).*/\1/p' | head -1
}

pick_wasm_clang() {
    local candidate major newest=''
    for candidate in clang clang-22 clang-21 clang-20 clang-19; do
        major="$(clang_major "$candidate" || true)"
        [[ -n "$major" ]] || continue
        if [[ "$major" -ge "$wasm_clang_minimum" ]]; then
            newest="$candidate"
            break
        fi
    done
    if [[ -z "$newest" ]]; then
        echo "The wasm build needs clang $wasm_clang_minimum or newer, and none was found on PATH." >&2
        echo "Install one (apt-get install clang-20 llvm-20) and run this again." >&2
        exit 1
    fi
    wasm_clang="$newest"
    wasm_clangxx="${newest/clang/clang++}"
    command -v "$wasm_clangxx" > /dev/null || wasm_clangxx='clang++'
    wasm_ar="${newest/clang/llvm-ar}"
    command -v "$wasm_ar" > /dev/null || wasm_ar='llvm-ar'
    assert_command "$wasm_ar" 'Install LLVM and put its bin directory on PATH.'
}

# An extraction that was interrupted leaves the headers behind without the
# archives the link needs, so what is checked for is what is actually linked
# against rather than the directory merely existing.
wasi_sysroot_is_complete() {
    [[ -d "$1/include" \
        && -d "$1/lib/$wasm_rust_target/noeh" \
        && -f "$1/lib/$wasm_rust_target/libsetjmp.a" ]]
}

export_wasi_toolchain() {
    local requested="${1:-}"
    unwrap_rustc_for_wasm
    pick_wasm_clang
    if ! rustup target list --installed | grep -qx "$wasm_rust_target"; then
        echo "Installing the $wasm_rust_target Rust target..."
        rustup target add "$wasm_rust_target"
    fi
    wasi_sysroot="$requested"
    if [[ -z "$wasi_sysroot" ]]; then
        local tools="$repository/target/tools"
        wasi_sysroot="$tools/wasi-sysroot"
        if ! wasi_sysroot_is_complete "$wasi_sysroot"; then
            local archive="$tools/wasi-sysroot.tar.gz"
            local extracted="$tools/wasi-sysroot-$wasi_sdk_version.0+m"
            local url="https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-$wasi_sdk_version/wasi-sysroot-$wasi_sdk_version.0+m.tar.gz"
            echo "Downloading the WASI sysroot from $url..."
            mkdir -p "$tools"
            rm -rf "$wasi_sysroot" "$extracted"
            curl --fail --location --output "$archive" "$url"
            tar -xzf "$archive" -C "$tools"
            mv "$extracted" "$wasi_sysroot"
            rm "$archive"
        fi
    fi
    if ! wasi_sysroot_is_complete "$wasi_sysroot"; then
        echo "The WASI sysroot at $wasi_sysroot has no lib/$wasm_rust_target to link against." >&2
        echo 'Delete it and run the build again to fetch it afresh, or pass --wasi-sysroot a complete one.' >&2
        exit 1
    fi
    wasi_sysroot="$(cd "$wasi_sysroot" && pwd)"
    local sysroot
    sysroot="$(native_path "$wasi_sysroot")"
    # cc and rustc both split these variables on whitespace, so a sysroot with a
    # space in its path arrives at clang as two flags that mean nothing.
    if [[ "$sysroot" == *' '* ]]; then
        echo "The WASI sysroot path contains a space, which cc and rustc split on: $sysroot" >&2
        echo 'Pass --wasi-sysroot a path without spaces, or move the checkout to one.' >&2
        exit 1
    fi
    echo "Building against the WASI sysroot at $sysroot"
    # -pthread is what marks the C objects as using atomics and bulk memory,
    # which is what lets them be linked into a module with a shared memory at
    # all. HarfBuzz itself is still left single-threaded, because only the
    # thread that draws shapes text and HB_NO_MT keeps it from paying for locks
    # nothing contends.
    local flags="--sysroot=$sysroot -isystem $sysroot/include/c++/v1 -pthread -mllvm -wasm-enable-sjlj -mllvm -wasm-use-legacy-eh=false -fno-exceptions -fno-rtti -DHB_NO_MT -O2 -w"
    export CC_wasm32_wasip1_threads="$wasm_clang"
    export CXX_wasm32_wasip1_threads="$wasm_clangxx"
    export AR_wasm32_wasip1_threads="$wasm_ar"
    export CFLAGS_wasm32_wasip1_threads="$flags"
    export CXXFLAGS_wasm32_wasip1_threads="$flags"
    export CXXSTDLIB_wasm32_wasip1_threads='c++'
    export HARFBUZZ_SYS_NO_PKG_CONFIG='1'
    # rustc links a cdylib with --no-entry, so lld strips the symbols a host
    # needs to lay out a thread's own storage. Exporting them is what lets it
    # turn the module into one that can be instantiated more than once.
    local exports=''
    local symbol
    for symbol in __heap_base __tls_base __tls_size __tls_align __wasm_init_tls; do
        exports+=" -C link-arg=--export=$symbol"
    done
    export RUSTFLAGS="-C link-arg=-L$sysroot/lib/$wasm_rust_target/noeh -C link-arg=$sysroot/lib/$wasm_rust_target/libsetjmp.a$exports"
}

# A plugin is its own cargo call: the app links wgpu with real backends and a
# guest links it with only the custom one, and a single call would unify the two
# into a guest that carries a backend it cannot use. It also gets its own
# profile, because an unoptimised guest is hundreds of megabytes of wasm that
# Cranelift then spends minutes compiling at every launch.
build_plugin_wasm() {
    local profile="$1" destination="$2"
    local wasm_profile='plugin'
    if [[ "$profile" == 'release' ]]; then
        wasm_profile='plugin-release'
    fi
    local arguments=(--target "$wasm_rust_target" --profile "$wasm_profile")
    local plugin selection=()
    for plugin in "${plugins[@]}"; do
        selection+=(-p "$plugin")
    done
    echo "Building ${#plugins[@]} plugins for $wasm_rust_target..."
    (
        export_wasi_toolchain "${wasi_sysroot:-}"
        cargo build "${arguments[@]}" "${selection[@]}"
    )
    local built="$repository/target/$wasm_rust_target/$wasm_profile"
    mkdir -p "$destination"
    for plugin in "${plugins[@]}"; do
        local module="$built/${plugin//-/_}.wasm"
        if [[ ! -f "$module" ]]; then
            echo "cargo did not produce $module" >&2
            exit 1
        fi
        cp -p "$module" "$destination/$plugin.wasm"
    done
}

# Cranelift compiles a plugin the first time the app opens it, which is seconds
# of work for a module this size and happens again on every machine the build
# lands on. Compiling it here instead leaves a .cwasm beside the .wasm that
# wasmtime maps straight in. The app still reads the .wasm whenever an artifact
# is missing or was made by a different wasmtime, so a plain cargo build stays
# usable; a stale artifact is one the app would fall back from, so the work is
# redone whenever the module or the compiler that produced it is newer.
precompiler=''

# Names the compiler a cargo call has already produced in the directory it
# builds into, so a build that compiled it alongside the app does not compile it
# a second time here.
use_precompiler() {
    local built="$1/precompile"
    if [[ -f "$built.exe" ]]; then
        built+='.exe'
    fi
    if [[ ! -f "$built" ]]; then
        echo "cargo did not produce $built" >&2
        exit 1
    fi
    precompiler="$built"
}

# The compiler runs on the machine the build runs on whatever the app is being
# built for, so only a build for this machine can hand it over from its own
# cargo call. A cross build compiles it here instead, where all-arch is what
# gives wasmtime the backend the app's architecture needs.
build_precompiler() {
    if [[ -n "$precompiler" ]]; then
        return
    fi
    echo 'Building the plugin compiler...'
    (cd "$repository" && cargo build --release -p block-wasm-host --features all-arch --bin precompile)
    use_precompiler "$repository/target/release"
}

precompile_plugin_wasm() {
    local directory="$1" triple="$2"
    build_precompiler
    local plugin module artifact stale=()
    for plugin in "${plugins[@]}"; do
        module="$directory/$plugin.wasm"
        artifact="$directory/$plugin.cwasm"
        if [[ -f "$artifact" && "$artifact" -nt "$module" && "$artifact" -nt "$precompiler" ]]; then
            continue
        fi
        stale+=("$module")
    done
    if [[ ${#stale[@]} -eq 0 ]]; then
        echo "Every plugin is already compiled for $triple"
        return
    fi
    echo "Compiling ${#stale[@]} plugins for $triple..."
    "$precompiler" --target "$triple" "${stale[@]}"
}

# sccache stands between cargo and rustc and answers a compilation from a shared
# object store whenever some other machine has already compiled that crate with
# those flags, which is most of what a cold checkout or a CI runner spends its
# first build on. Every profile in Cargo.toml already turns incremental
# compilation off, so nothing is given up by routing rustc through it.
#
# The store is a Bunny storage zone. Its read-only key is in the clear on
# purpose: a fresh clone and a pull request from a fork can both read what the
# project has already built without holding a secret. Writing needs the key that
# is not public, which arrives as BUNNY_SCCACHE_PASSWORD, from a repository
# secret in CI and from the environment of whoever has it locally. Without it
# the cache is read-only, which costs a miss nothing but a locally reported
# write error.
sccache_version='0.18.0'
sccache_directory="$repository/target/tools/sccache"
sccache_bucket='sccache'
sccache_read_only_password='11737dc0-6417-4dcc-a076dc81ac00-71a7-461f'

# internal/install-sccache.sh puts one under target, and CI installs one on
# PATH; either will do. Cargo spawns the wrapper itself rather than through a
# shell, so the one on PATH is left as a bare name for cargo to resolve, and
# only the installed copy travels as a path, which on Windows has to be a form
# a native program can open.
find_sccache() {
    if [[ -x "$sccache_directory/sccache" ]]; then
        native_path "$sccache_directory/sccache"
    elif command -v sccache > /dev/null; then
        echo 'sccache'
    fi
}

# Sourcing this file is what turns the cache on, so every script that builds
# gets it without having to ask. A machine with no sccache installed, one whose
# cargo is already pointed at a wrapper, and one that set BE3_NO_SCCACHE are all
# left exactly as they were.
configure_sccache() {
    if [[ -n "${RUSTC_WRAPPER:-}" || -n "${BE3_NO_SCCACHE:-}" ]]; then
        return
    fi
    local binary
    binary="$(find_sccache)"
    if [[ -z "$binary" ]]; then
        return
    fi

    export RUSTC_WRAPPER="$binary"
    export SCCACHE_BUCKET="$sccache_bucket"
    export SCCACHE_ENDPOINT='https://de-s3.storage.bunnycdn.com'
    # Bunny serves one region per endpoint and pays no attention to this, but
    # the S3 signature it does check is computed over the region name, so both
    # ends have to spell it the same way.
    export SCCACHE_REGION='de'
    # Bunny's S3 gateway takes the storage zone as the access key and a zone
    # password as the secret.
    export AWS_ACCESS_KEY_ID="$sccache_bucket"
    if [[ -n "${BUNNY_SCCACHE_PASSWORD:-}" ]]; then
        export AWS_SECRET_ACCESS_KEY="$BUNNY_SCCACHE_PASSWORD"
        export SCCACHE_S3_RW_MODE='READ_WRITE'
    else
        export AWS_SECRET_ACCESS_KEY="$sccache_read_only_password"
        # Left out, sccache would upload every miss and take a 403 for it.
        export SCCACHE_S3_RW_MODE='READ_ONLY'
    fi
}

configure_sccache

# What a Windows job cannot cache, and why it is only the wasm half of one.
#
# Cargo lists every feature a crate declares in a single --check-cfg when it
# compiles that crate, and web-sys declares four thousand of them: one argument
# past the 32k a Windows command line holds. Cargo stays under the cap by
# handing rustc a response file, but sccache reads that file and spawns rustc
# itself with every argument written out, so the compile dies with "The
# filename or extension is too long".
#
# Nothing on this side can shorten that argument, so a wasm cargo call on
# Windows goes straight to rustc. The native build keeps the cache, which is
# where a Windows job spends most of its time and where nothing compiled comes
# near the cap.
unwrap_rustc_for_wasm() {
    if [[ "${OS:-}" == 'Windows_NT' ]]; then
        unset RUSTC_WRAPPER
    fi
}
