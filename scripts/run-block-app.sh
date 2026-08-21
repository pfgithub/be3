#!/usr/bin/env bash

set -euo pipefail

repository="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository"

./scripts/fetch-pdfium.sh
cargo build -p counter --bin counter-host
cargo build -p checklist --bin checklist-host
cargo build -p hotbar --bin hotbar-host
cargo build -p image_block --bin image_block-host
cargo build -p workspace_index --bin workspace_index-host
cargo run -p block-app "$@"
