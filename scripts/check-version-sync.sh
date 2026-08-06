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

# Manifest self-consistency: while the support policy is exact-pin, every
# version field in the manifest moves together with the release version, and
# the installation block must stay structurally complete so the skill
# preflight can always self-install.
python3 - "$EXPECTED" <<'PY'
import json, sys

expected = sys.argv[1]
manifest = json.load(open("plugin/skills/computer-pilot/compatibility.json"))

problems = []
cli = manifest.get("cli", {})
for field in ("version", "tested_version", "minimum_version"):
    actual = cli.get(field)
    if actual != expected:
        problems.append(f"cli.{field} is {actual!r}, expected {expected!r}")

installation = manifest.get("installation")
if not installation:
    problems.append("installation block is missing")
else:
    for field in ("repository", "asset_template", "installer", "install_root", "command_path"):
        if not installation.get(field):
            problems.append(f"installation.{field} is missing or empty")
    signing = installation.get("signing") or {}
    if signing.get("identifier") != "com.linlab.computer-pilot.cu":
        problems.append("installation.signing.identifier must be com.linlab.computer-pilot.cu")
    if signing.get("required_status") not in ("developer-id-notarized", "ad-hoc-unsigned"):
        problems.append("installation.signing.required_status is invalid")

if manifest.get("schema_version") != 2:
    problems.append(f"schema_version is {manifest.get('schema_version')!r}, expected 2")

if problems:
    for problem in problems:
        print(f"Manifest inconsistency: {problem}", file=sys.stderr)
    sys.exit(1)
PY

echo "All release surfaces report $EXPECTED"
