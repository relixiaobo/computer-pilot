#!/bin/bash

set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "Usage: build-release-assets.sh <version> <signed-cu-path> <output-dir>" >&2
  exit 1
fi

VERSION="$1"
BINARY="$2"
OUTPUT_DIR="$3"
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

if [[ ! -x "$BINARY" ]]; then
  echo "Signed binary is missing or not executable: $BINARY" >&2
  exit 1
fi
if [[ ! "$OUTPUT_DIR" = /* ]]; then
  echo "Output directory must be absolute" >&2
  exit 1
fi

mkdir -p "$OUTPUT_DIR"
STAGING="$(mktemp -d)"
trap 'rm -rf "$STAGING"' EXIT

ARCHIVE_ROOT="$STAGING/computer-pilot-v$VERSION-macos-arm64"
mkdir -p "$ARCHIVE_ROOT"
cp "$BINARY" "$ARCHIVE_ROOT/cu"
cp "$ROOT_DIR/README.md" "$ARCHIVE_ROOT/README.md"
cp "$ROOT_DIR/plugin/skills/computer-pilot/compatibility.json" "$ARCHIVE_ROOT/compatibility.json"

BINARY_ARCHIVE="computer-pilot-v$VERSION-macos-arm64.tar.gz"
SKILL_ARCHIVE="computer-pilot-skill-v$VERSION.tar.gz"
PLUGIN_ARCHIVE="computer-pilot-plugin-v$VERSION.tar.gz"

tar -C "$STAGING" -czf "$OUTPUT_DIR/$BINARY_ARCHIVE" "computer-pilot-v$VERSION-macos-arm64"
tar -C "$ROOT_DIR/plugin/skills" -czf "$OUTPUT_DIR/$SKILL_ARCHIVE" computer-pilot
tar -C "$ROOT_DIR" -czf "$OUTPUT_DIR/$PLUGIN_ARCHIVE" plugin
cp "$BINARY" "$OUTPUT_DIR/cu-arm64"

checksum() {
  local filename="$1"
  local digest
  digest="$(shasum -a 256 "$OUTPUT_DIR/$filename" | awk '{print $1}')"
  printf '%s  %s\n' "$digest" "$filename" >"$OUTPUT_DIR/$filename.sha256"
  printf '%s' "$digest"
}

BINARY_SHA="$(checksum "$BINARY_ARCHIVE")"
SKILL_SHA="$(checksum "$SKILL_ARCHIVE")"
PLUGIN_SHA="$(checksum "$PLUGIN_ARCHIVE")"
RAW_SHA="$(checksum cu-arm64)"

cat >"$OUTPUT_DIR/release-index.json" <<EOF
{
  "schema_version": 1,
  "version": "$VERSION",
  "integration_model": "skill-shell-cli",
  "platforms": ["macos-arm64"],
  "unsupported_platforms": ["macos-x86_64"],
  "assets": [
    {"name": "$BINARY_ARCHIVE", "type": "application/gzip", "sha256": "$BINARY_SHA"},
    {"name": "cu-arm64", "type": "application/octet-stream", "sha256": "$RAW_SHA"},
    {"name": "$SKILL_ARCHIVE", "type": "application/gzip", "sha256": "$SKILL_SHA"},
    {"name": "$PLUGIN_ARCHIVE", "type": "application/gzip", "sha256": "$PLUGIN_SHA"}
  ]
}
EOF
checksum release-index.json >/dev/null

echo "Release assets created in $OUTPUT_DIR"
