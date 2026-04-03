#!/bin/bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cargo build --manifest-path "$ROOT_DIR/Cargo.toml" --release
echo "Built: $ROOT_DIR/target/release/cu"
