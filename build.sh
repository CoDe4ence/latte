#!/usr/bin/env bash
# Builds Latte for FigCraft and puts the module beside its manifest in dist/, ready to import
# (Plugins ▸ Development ▸ Import plugin from manifest… → dist/figcraft-plugin.json).
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown
mkdir -p dist
cp target/wasm32-unknown-unknown/release/latte.wasm dist/latte.wasm
cp figcraft-plugin.json dist/figcraft-plugin.json
echo "dist/latte.wasm: $(wc -c < dist/latte.wasm | tr -d ' ') bytes"
