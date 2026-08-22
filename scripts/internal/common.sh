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

write_plugin_index() {
    local directory="$1"
    shift
    local ids=("$@")
    local index separator
    {
        echo '['
        for index in "${!ids[@]}"; do
            separator=','
            if [[ $index -eq $((${#ids[@]} - 1)) ]]; then
                separator=''
            fi
            echo "  \"${ids[$index]}\"$separator"
        done
        echo ']'
    } > "$directory/index.json"
}

profile_directory() {
    if [[ "$1" == 'release' ]]; then
        echo 'release'
    else
        echo 'debug'
    fi
}
