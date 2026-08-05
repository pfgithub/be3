#!/usr/bin/env bash
#
# Lints, formats and tests the workspace.
#
# Runs clippy, rustfmt and the test suite over the whole workspace. This is
# the one command to run after making changes.
#
# By default clippy applies the fixes it can and rustfmt rewrites the
# sources. With --check nothing is written: clippy and rustfmt only report,
# which is what CI wants. Either way the run fails if any clippy warning
# survives.
#
# Usage:
#   ./scripts/verify.sh
#   ./scripts/verify.sh --check

set -euo pipefail

check=false
if [[ "${1:-}" == "--check" ]]; then
    check=true
fi

repository="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository"

# Tests and examples are linted too, so a warning cannot hide in them.
clippy_targets=(--workspace --all-targets --all-features)

if $check; then
    echo 'Checking for clippy warnings...'
    cargo clippy "${clippy_targets[@]}" -- -D warnings
else
    echo 'Applying clippy fixes...'
    # The tree is expected to be mid-change; fixing only a pristine checkout
    # would make the script useless during development.
    cargo clippy --fix --allow-dirty --allow-staged "${clippy_targets[@]}" -- -D warnings
fi

echo 'Formatting...'
format_arguments=(--all)
if $check; then
    format_arguments+=(--check)
fi
cargo fmt "${format_arguments[@]}"

echo 'Running tests...'
test_arguments=(--workspace)
if $check; then
    test_arguments+=(--profile ci)
fi
cargo nextest run "${test_arguments[@]}"

echo ''
echo 'All checks passed.'
