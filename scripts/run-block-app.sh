#!/usr/bin/env bash

set -euo pipefail

repository="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository"

cargo build -p counter --bin counter-host
cargo run -p block-app "$@"
