cleanup() {
    echo "Killing"
    kill 0
}

trap cleanup INT

echo "Note: set BE3_DOMAIN_NAME / BE3_BACKEND_URL"
: "${BE3_DOMAIN_NAME:=blocks.pfg.pw}"
: "${BE3_BACKEND_URL:=127.0.0.1:9090}"
echo "BE3_DOMAIN_NAME=$BE3_DOMAIN_NAME"
echo "BE3_BACKEND_URL=$BE3_BACKEND_URL"

echo "Building website & server..."
./scripts/build-block-web.sh &
cargo build -p block-server &
wait

echo "Running servers..."
cargo run -p block-server -- --disable-registration &
sudo BE3_DOMAIN_NAME="$BE3_DOMAIN_NAME" BE3_BACKEND_URL="$BE3_BACKEND_URL" caddy run --config ./scripts/web/Caddyfile &
wait
