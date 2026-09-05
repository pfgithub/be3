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
# leaves them where cargo put them, in games_directory. The modules are
# interpreted wherever the app runs, so they are built the same way for every
# target.
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
    (cd "$repository" && cargo build "${arguments[@]}")

    games_directory="$repository/target/wasm32-unknown-unknown/$profile"
    for game in "${games[@]}"; do
        if [[ ! -f "$games_directory/$game.wasm" ]]; then
            echo "cargo did not produce $games_directory/$game.wasm" >&2
            exit 1
        fi
    done
}

# The app reads games.json beside itself, whose entries are paths relative to
# the index. Every build stages the modules beside the app it built, so the
# index says the same thing wherever it is read and a plugin asking the host
# for one of them names a path inside the app's own directory.
write_games_index() {
    local file="$1" prefix="$2"
    local entries=() game
    for game in "${games[@]}"; do
        entries+=("$prefix$game.wasm")
    done
    write_index "$file" "${entries[@]}"
}

stage_games() {
    local directory="$1"
    rm -rf "$directory"
    mkdir -p "$directory"
    local game
    for game in "${games[@]}"; do
        cp "$games_directory/$game.wasm" "$directory/$game.wasm"
    done
}

# Both the WASI toolchain and the guest flags a plugin needs to compile without
# wasm-bindgen. The web build exports the same environment for its own cargo
# call; a plugin built for wasmtime is the same target with none of the glue.
wasi_sdk_version='33'
wasm_rust_target='wasm32-wasip1-threads'

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
    assert_command clang 'Install LLVM and put its bin directory on PATH.'
    assert_command llvm-ar 'Install LLVM and put its bin directory on PATH.'
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
    export CC_wasm32_wasip1_threads='clang'
    export CXX_wasm32_wasip1_threads='clang++'
    export AR_wasm32_wasip1_threads='llvm-ar'
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

build_precompiler() {
    if [[ -n "$precompiler" ]]; then
        return
    fi
    echo 'Building the plugin compiler...'
    (cd "$repository" && cargo build --release -p block-wasm-host --features all-arch --example precompile)
    local built="$repository/target/release/examples/precompile"
    if [[ -f "$built.exe" ]]; then
        built+='.exe'
    fi
    if [[ ! -f "$built" ]]; then
        echo "cargo did not produce $built" >&2
        exit 1
    fi
    precompiler="$built"
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
