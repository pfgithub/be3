#!/usr/bin/env bash

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

profile='debug'
build_arguments=()
application_arguments=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --release)
            profile='release'
            build_arguments+=(--release)
            shift
            ;;
        --)
            shift
            application_arguments=("$@")
            break
            ;;
        *)
            echo "Unknown argument: $1. Pass application arguments after --." >&2
            exit 1
            ;;
    esac
done

"$internal/build-native.sh" --no-server "${build_arguments[@]}"

cd "$repository"
exec "$repository/target/$profile/block-app" "${application_arguments[@]}"
