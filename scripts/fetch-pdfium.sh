#!/usr/bin/env bash

set -euo pipefail

release=false
target=''
target_given=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        --release)
            release=true
            shift
            ;;
        --target)
            target="$2"
            target_given=true
            shift 2
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

assert_command() {
    if ! command -v "$1" > /dev/null; then
        echo "$1 was not found on PATH. $2" >&2
        exit 1
    fi
}

assert_command curl 'Install curl.'
assert_command tar 'Install tar.'

if [[ -z "$target" ]]; then
    case "$(uname -s)" in
        Linux) os='linux' ;;
        Darwin) os='mac' ;;
        MINGW*|MSYS*|CYGWIN*) os='windows' ;;
        *)
            echo "Unsupported platform: $(uname -s)" >&2
            exit 1
            ;;
    esac
    case "$(uname -m)" in
        x86_64|amd64) cpu='x86_64' ;;
        aarch64|arm64) cpu='aarch64' ;;
        *)
            echo "Unsupported architecture: $(uname -m)" >&2
            exit 1
            ;;
    esac
    case "$os" in
        linux) target="$cpu-unknown-linux-gnu" ;;
        mac) target="$cpu-apple-darwin" ;;
        windows) target="$cpu-pc-windows-msvc" ;;
    esac
fi

case "$target" in
    x86_64-unknown-linux-gnu)
        asset='pdfium-linux-x64'
        library_path='lib/libpdfium.so'
        library_name='libpdfium.so'
        ;;
    aarch64-unknown-linux-gnu)
        asset='pdfium-linux-arm64'
        library_path='lib/libpdfium.so'
        library_name='libpdfium.so'
        ;;
    x86_64-apple-darwin)
        asset='pdfium-mac-x64'
        library_path='lib/libpdfium.dylib'
        library_name='libpdfium.dylib'
        ;;
    aarch64-apple-darwin)
        asset='pdfium-mac-arm64'
        library_path='lib/libpdfium.dylib'
        library_name='libpdfium.dylib'
        ;;
    x86_64-pc-windows-msvc|x86_64-pc-windows-gnu)
        asset='pdfium-win-x64'
        library_path='bin/pdfium.dll'
        library_name='pdfium.dll'
        ;;
    aarch64-pc-windows-msvc|aarch64-pc-windows-gnu)
        asset='pdfium-win-arm64'
        library_path='bin/pdfium.dll'
        library_name='pdfium.dll'
        ;;
    *)
        echo "No PDFium binary is known for target $target" >&2
        exit 1
        ;;
esac

repository="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
profile_directory='debug'
if $release; then
    profile_directory='release'
fi
if $target_given; then
    output_directory="$repository/target/$target/$profile_directory"
else
    output_directory="$repository/target/$profile_directory"
fi

mkdir -p "$output_directory"
destination="$output_directory/$library_name"
if [[ -f "$destination" ]]; then
    echo "PDFium is already present at $destination"
    exit 0
fi

tools_directory="$repository/target/tools"
mkdir -p "$tools_directory"
archive="$tools_directory/$asset.tgz"
url="https://github.com/bblanchon/pdfium-binaries/releases/latest/download/$asset.tgz"
echo "Downloading PDFium from $url..."
curl --fail --location --output "$archive" "$url"

extract_directory="$tools_directory/$asset"
rm -rf "$extract_directory"
mkdir -p "$extract_directory"
tar -xzf "$archive" -C "$extract_directory"
rm "$archive"

cp "$extract_directory/$library_path" "$destination"
echo "Installed PDFium at $destination"
echo "PDFium is distributed under the licenses listed in $extract_directory/LICENSE"
