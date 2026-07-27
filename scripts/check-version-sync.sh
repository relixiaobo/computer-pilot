#!/bin/bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

EXPECTED="${1:-$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)}"

read_json_version() {
  sed -n 's/^[[:space:]]*"version":[[:space:]]*"\([^"]*\)".*/\1/p' "$1" | head -1
}

check() {
  local file="$1"
  local actual="$2"
  if [[ "$actual" != "$EXPECTED" ]]; then
    echo "Version mismatch: $file has '$actual', expected '$EXPECTED'" >&2
    exit 1
  fi
}

check Cargo.toml "$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)"
check plugin/.claude-plugin/plugin.json "$(read_json_version plugin/.claude-plugin/plugin.json)"
check plugin/package.json "$(read_json_version plugin/package.json)"
check .claude-plugin/marketplace.json "$(read_json_version .claude-plugin/marketplace.json)"
check plugin/skills/computer-pilot/compatibility.json "$(read_json_version plugin/skills/computer-pilot/compatibility.json)"

echo "All release surfaces report $EXPECTED"
