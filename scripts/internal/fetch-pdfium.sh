#!/usr/bin/env bash

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

triple=''
output=''
while [[ $# -gt 0 ]]; do
    case "$1" in
        --triple)
            triple="$2"
            shift 2
            ;;
        --output)
            output="$2"
            shift 2
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

assert_command curl 'Install curl.'
assert_command tar 'Install tar.'

if [[ -z "$triple" || -z "$output" ]]; then
    echo 'Usage: fetch-pdfium.sh --triple TRIPLE --output DIRECTORY' >&2
    exit 1
fi

case "$triple" in
    x86_64-unknown-linux-gnu)
        asset='pdfium-linux-x64'
        library_path='lib/libpdfium.so'
        ;;
    aarch64-unknown-linux-gnu)
        asset='pdfium-linux-arm64'
        library_path='lib/libpdfium.so'
        ;;
    x86_64-apple-darwin)
        asset='pdfium-mac-x64'
        library_path='lib/libpdfium.dylib'
        ;;
    aarch64-apple-darwin)
        asset='pdfium-mac-arm64'
        library_path='lib/libpdfium.dylib'
        ;;
    x86_64-pc-windows-msvc|x86_64-pc-windows-gnu)
        asset='pdfium-win-x64'
        library_path='bin/pdfium.dll'
        ;;
    aarch64-pc-windows-msvc|aarch64-pc-windows-gnu)
        asset='pdfium-win-arm64'
        library_path='bin/pdfium.dll'
        ;;
    *)
        echo "No PDFium binary is known for target $triple" >&2
        exit 1
        ;;
esac
library_name="$(basename "$library_path")"

mkdir -p "$output"
destination="$output/$library_name"
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
