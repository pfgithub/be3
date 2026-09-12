#!/usr/bin/env bash

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

profile='debug'
smoke=false
build_arguments=()
application_arguments=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --release)
            profile='release'
            build_arguments+=(--release)
            shift
            ;;
        --smoke)
            smoke=true
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
if $smoke; then
    if [[ ${#application_arguments[@]} -ne 0 ]]; then
        echo '--smoke does not accept application arguments' >&2
        exit 1
    fi
    assert_command xvfb-run 'Install xvfb to run the native startup smoke check.'
    assert_command timeout 'Install GNU coreutils to run the native startup smoke check.'
    smoke_data="$(mktemp -d)"
    trap 'rm -rf "$smoke_data"' EXIT
    set +e
    XDG_DATA_HOME="$smoke_data" xvfb-run -a \
        timeout --kill-after=5s 10s "$repository/target/$profile/block-app"
    status=$?
    set -e
    if [[ $status -eq 124 ]]; then
        echo 'Native startup smoke check passed.'
        exit 0
    fi
    if [[ $status -eq 0 ]]; then
        echo 'Native startup smoke check failed: the app exited before ten seconds.' >&2
        exit 1
    fi
    echo "Native startup smoke check failed with status $status." >&2
    exit "$status"
fi
exec "$repository/target/$profile/block-app" "${application_arguments[@]}"
