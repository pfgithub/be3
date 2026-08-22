#!/usr/bin/env bash

set -euo pipefail

repository="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository"

./scripts/fetch-pdfium.sh

manifest_field() {
    sed -n "s/.*\"$2\"[[:space:]]*:[[:space:]]*\"\([^\"]*\)\".*/\1/p" "$1" | head -1
}

# The app discovers plugins in plugins/<plugin id> beside its executable, so
# stage every package under crates/editors there.
plugins_directory="$repository/target/debug/plugins"
rm -rf "$plugins_directory"
for manifest in "$repository"/crates/editors/*/manifest.json; do
    [[ -f "$manifest" ]] || continue
    plugin="$(basename "$(dirname "$manifest")")"
    plugin_id="$(manifest_field "$manifest" id)"
    if [[ -z "$plugin_id" ]]; then
        echo "$manifest has no plugin id" >&2
        exit 1
    fi
    cargo build -p "$plugin" --bin "$plugin-host"
    plugin_directory="$plugins_directory/$plugin_id"
    mkdir -p "$plugin_directory"
    cp "$manifest" "$plugin_directory/manifest.json"
    cp "$repository/target/debug/$plugin-host" "$plugin_directory/$plugin-host"
done

cargo run -p block-app "$@"
