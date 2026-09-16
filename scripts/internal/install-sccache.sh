#!/usr/bin/env bash
#
# Puts the pinned sccache under target/tools, where common.sh looks for it.
#
# A prebuilt release is fetched rather than built with `cargo install`, because
# compiling sccache costs more than the first build it accelerates saves. The
# version is common.sh's, so a machine and a CI runner fill the same cache with
# the same compiler cache.
#
# Usage:
#   install-sccache.sh

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

if [[ $# -ne 0 ]]; then
    echo 'Usage: install-sccache.sh' >&2
    exit 1
fi

binary="$sccache_directory/sccache"
if [[ -x "$binary" ]] && [[ "$("$binary" --version 2> /dev/null)" == "sccache $sccache_version" ]]; then
    echo "sccache $sccache_version is already installed at $binary"
    exit 0
fi

case "$(uname -m)" in
    x86_64 | amd64)
        architecture='x86_64'
        ;;
    aarch64 | arm64)
        architecture='aarch64'
        ;;
    *)
        echo "sccache publishes no release for $(uname -m), so builds here will not share a cache." >&2
        exit 1
        ;;
esac

# The musl builds run on any glibc a machine happens to have, which the gnu
# ones do not.
release="sccache-v$sccache_version-$architecture-unknown-linux-musl"
url="https://github.com/mozilla/sccache/releases/download/v$sccache_version/$release.tar.gz"
tools="$repository/target/tools"
archive="$tools/sccache.tar.gz"

echo "Downloading sccache from $url..."
mkdir -p "$tools"
rm -rf "$sccache_directory" "$tools/$release"
curl --fail --location --output "$archive" "$url"
tar -xzf "$archive" -C "$tools"
mv "$tools/$release" "$sccache_directory"
rm "$archive"

echo "Installed $("$binary" --version) at $binary"
