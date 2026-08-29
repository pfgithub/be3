repository="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
internal="$repository/scripts/internal"

assert_command() {
    if ! command -v "$1" > /dev/null; then
        echo "$1 was not found on PATH. $2" >&2
        exit 1
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
# the index. A build that has to bundle the modules — the browser, an APK, a
# packaged directory — copies them and points the index at the copies; a build
# that runs out of target/ points it straight at what cargo compiled.
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
        wasi_sysroot="$repository/target/tools/wasi-sysroot"
        if [[ ! -d "$wasi_sysroot/include" ]]; then
            local archive="$repository/target/tools/wasi-sysroot.tar.gz"
            local url="https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-$wasi_sdk_version/wasi-sysroot-$wasi_sdk_version.0+m.tar.gz"
            echo "Downloading the WASI sysroot from $url..."
            mkdir -p "$repository/target/tools"
            curl --fail --location --output "$archive" "$url"
            tar -xzf "$archive" -C "$repository/target/tools"
            mv "$repository/target/tools/wasi-sysroot-$wasi_sdk_version.0+m" "$wasi_sysroot"
            rm "$archive"
        fi
    fi
    if [[ ! -d "$wasi_sysroot/include" ]]; then
        echo "no WASI sysroot at $wasi_sysroot" >&2
        exit 1
    fi
    wasi_sysroot="$(cd "$wasi_sysroot" && pwd)"
    local flags="--sysroot=$wasi_sysroot -isystem $wasi_sysroot/include/c++/v1 -pthread -mllvm -wasm-enable-sjlj -mllvm -wasm-use-legacy-eh=false -fno-exceptions -fno-rtti -DHB_NO_MT -O2 -w"
    export CC_wasm32_wasip1_threads='clang'
    export CXX_wasm32_wasip1_threads='clang++'
    export AR_wasm32_wasip1_threads='llvm-ar'
    export CFLAGS_wasm32_wasip1_threads="$flags"
    export CXXFLAGS_wasm32_wasip1_threads="$flags"
    export CXXSTDLIB_wasm32_wasip1_threads='c++'
    export HARFBUZZ_SYS_NO_PKG_CONFIG='1'
    local exports=''
    local symbol
    for symbol in __heap_base __tls_base __tls_size __tls_align __wasm_init_tls; do
        exports+=" -C link-arg=--export=$symbol"
    done
    export RUSTFLAGS="-C link-arg=-L$wasi_sysroot/lib/$wasm_rust_target/noeh -C link-arg=$wasi_sysroot/lib/$wasm_rust_target/libsetjmp.a$exports"
}

# A plugin for wasmtime is its own cargo call: the app links wgpu with real
# backends and a guest links it with only the custom one, and a single call
# would unify the two into a guest that carries a backend it cannot use. It also
# gets its own profile, because an unoptimised guest is hundreds of megabytes of
# wasm that Cranelift then spends minutes compiling at every launch.
build_plugin_wasm() {
    local profile="$1" destination="$2"
    local wasm_plugins=() plugin
    for plugin in "${plugins[@]}"; do
        if [[ -n "$(manifest_field "$(plugin_manifest "$plugin")" wasm)" ]]; then
            wasm_plugins+=("$plugin")
        fi
    done
    if [[ ${#wasm_plugins[@]} -eq 0 ]]; then
        return 0
    fi
    local wasm_profile='plugin'
    if [[ "$profile" == 'release' ]]; then
        wasm_profile='plugin-release'
    fi
    local arguments=(
        --target "$wasm_rust_target"
        --profile "$wasm_profile"
        --no-default-features
        --features hosted
    )
    local selection=()
    for plugin in "${wasm_plugins[@]}"; do
        selection+=(-p "$plugin")
    done
    echo "Building ${#wasm_plugins[@]} plugins for $wasm_rust_target..."
    (
        export_wasi_toolchain "${wasi_sysroot:-}"
        cargo build "${arguments[@]}" "${selection[@]}"
    )
    local built="$repository/target/$wasm_rust_target/$wasm_profile"
    mkdir -p "$destination"
    for plugin in "${wasm_plugins[@]}"; do
        local module="$built/${plugin//-/_}.wasm"
        if [[ ! -f "$module" ]]; then
            echo "cargo did not produce $module" >&2
            exit 1
        fi
        cp "$module" "$destination/$plugin.wasm"
    done
}
