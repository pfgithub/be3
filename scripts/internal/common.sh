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
