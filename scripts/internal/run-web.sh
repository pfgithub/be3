#!/usr/bin/env bash

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

build_arguments=("$@")

assert_command caddy 'Install Caddy from https://caddyserver.com/docs/install.'

: "${BE3_DOMAIN_NAME:=blocks.pfg.pw}"
: "${BE3_BACKEND_URL:=127.0.0.1:9090}"
echo "BE3_DOMAIN_NAME=$BE3_DOMAIN_NAME"
echo "BE3_BACKEND_URL=$BE3_BACKEND_URL"

cd "$repository"

cleanup() {
    kill 0
    wait
}
trap cleanup INT

echo 'Building the website and the server...'
"$internal/build-web.sh" "${build_arguments[@]}" &
cargo build -p block-server &
wait

echo 'Running the servers...'
cargo run -p block-server -- --disable-registration &
sudo BE3_DOMAIN_NAME="$BE3_DOMAIN_NAME" BE3_BACKEND_URL="$BE3_BACKEND_URL" \
    caddy run --config "$internal/web/Caddyfile" &
wait
