#!/usr/bin/env bash
# Builds the static web app into dist/ (or the directory given as $1).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="${1:-$root/dist}"

cargo build --manifest-path "$root/Cargo.toml" --release --lib --target wasm32-unknown-unknown
rm -rf "$out"
mkdir -p "$out"
cp "$root"/web/* "$out"/
cp "$root/target/wasm32-unknown-unknown/release/binkies.wasm" "$out"/
echo "built $out"
