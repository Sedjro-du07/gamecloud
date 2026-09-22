#!/usr/bin/env bash
# Deploy GameCloud OS to Railway: compile here, upload the binaries.
#
#   ./deploy/railway-up.sh          # build + upload web and bot
#
# Railway does not compile anything: it wraps the uploaded binaries in
# deploy/railway.Dockerfile. Migrations are embedded in the web binary and
# run against the production database when it starts.
set -euo pipefail
cd "$(dirname "$0")/.."

export CARGO_INCREMENTAL=0
cargo build --release -p gamecloud-bot
cargo leptos build --release

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/bin"
cp target/release/gamecloud-web target/release/gamecloud-bot "$stage/bin/"
cp -r target/site "$stage/site"
cp deploy/railway.Dockerfile "$stage/Dockerfile"

for service in web bot; do
  npx -y @railway/cli up "$stage" --path-as-root --service "$service" --detach
done
echo "Envoyé. Suivi : npx @railway/cli service list"
