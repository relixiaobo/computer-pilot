#!/bin/bash
# Reproduce the tracked local macOSWorld baseline cohort.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
MACOSWORLD_ROOT="${MACOSWORLD_ROOT:-}"
CU="${CU:-$ROOT_DIR/target/release/cu}"
MODEL="${MODEL:-gpt-6.1-sol}"
PINNED_REVISION="e2ca8334b3765537e5c1c428ce57990248a1bb5f"

if [[ -z "$MACOSWORLD_ROOT" || ! -d "$MACOSWORLD_ROOT/.git" ]]; then
  echo "Set MACOSWORLD_ROOT to a macOSWorld checkout at $PINNED_REVISION." >&2
  exit 1
fi
if [[ "$(git -C "$MACOSWORLD_ROOT" rev-parse HEAD)" != "$PINNED_REVISION" ]]; then
  echo "MACOSWORLD_ROOT must be pinned to $PINNED_REVISION." >&2
  exit 1
fi
[[ -x "$CU" ]] || { echo "cu binary not found: $CU" >&2; exit 1; }

TASK_IDS=(
  259bcb80-aa2b-8291-1d11-6ff4ae36f8cb
  1ef109cd-fea2-d2d5-b4e4-fbbad6689b2a
  0e224b29-14f7-3629-ee27-13a9789a997d
)
TASK_LIST="$(mktemp "${TMPDIR:-/tmp}/computer-pilot-macosworld.XXXXXX")"
cleanup() { rm -f "$TASK_LIST"; }
trap cleanup EXIT

for task_id in "${TASK_IDS[@]}"; do
  task_file="$(find "$MACOSWORLD_ROOT/tasks" -type f -name "$task_id.json" -print -quit)"
  [[ -n "$task_file" ]] || { echo "Task not found: $task_id" >&2; exit 1; }
  printf '%s\n' "$task_file" >> "$TASK_LIST"
done

if [[ -f "$ROOT_DIR/.env" ]]; then
  set -a
  # shellcheck disable=SC1091
  . "$ROOT_DIR/.env"
  set +a
fi
export TASK_LIST MODELS="$MODEL" CU
python3 "$ROOT_DIR/tests/macosworld/run_selected.py"
