#!/bin/bash
# Test: plugin/skills/computer-pilot/scripts/install-native.sh
#
# Behavior tests (Rule 1): each case constructs the real scenario the
# installer was built for — sandboxed fixed-path install, idempotent
# convergence, atomic upgrade (inode changes, realpath does not), corrupted
# and hostile archives, unmanaged-command refusal, and PATH shadow detection.
# All installs run against --asset-directory built locally; no network.
source "$(dirname "$0")/helpers.sh"

INSTALLER="$ROOT_DIR/plugin/skills/computer-pilot/scripts/install-native.sh"
BUILD_ASSETS="$ROOT_DIR/scripts/build-release-assets.sh"
REAL_VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -1)"

SANDBOX="$(mktemp -d "${TMPDIR:-/tmp}/cu-installer-test.XXXXXX")"
cleanup_sandbox() { rm -rf "$SANDBOX"; }
trap 'cleanup_sandbox; cleanup_run' EXIT

INSTALL_ROOT="$SANDBOX/data/computer-pilot"
BIN_DIR="$SANDBOX/bin"
FIXED="$INSTALL_ROOT/bin/cu"
RESTRICTED_PATH="/usr/bin:/bin:/usr/sbin:/sbin"

# Run the installer with a controlled PATH so the user's real cu never leaks
# into shadow detection. Sets OUT / ERR / EXIT.
run_installer() {
  EXIT=0
  OUT=$(env PATH="$RESTRICTED_PATH" HOME="$SANDBOX/home" \
    sh "$INSTALLER" "$@" 2>/tmp/cu-test-stderr) || EXIT=$?
  ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
}

out_field() {
  printf '%s\n' "$OUT" | sed -n "s/^$1=//p" | head -1
}

assert_err_code() {
  local name="$1" code="$2"
  if [[ $EXIT -ne 0 && "$ERR" == *"code=$code"* ]]; then
    _pass "$name"
  else
    _fail "$name" "expected failure code=$code, got exit=$EXIT err=${ERR:0:200}"
  fi
}

mkdir -p "$SANDBOX/home"

section "installer — asset preparation"

ASSETS_REAL="$SANDBOX/assets-real"
if bash "$BUILD_ASSETS" "$REAL_VERSION" "$CU" "$ASSETS_REAL" >/dev/null 2>&1; then
  _pass "release assets built for v$REAL_VERSION"
else
  _fail "release assets built for v$REAL_VERSION" "build-release-assets.sh failed"
  summary
  exit 1
fi

# A fake next release: a shell stub that reports version 9.9.9, packaged with
# the same asset pipeline. Exercises upgrade without needing a second build.
FAKE_BIN="$SANDBOX/fake-cu"
printf '#!/bin/sh\necho "cu 9.9.9"\n' >"$FAKE_BIN"
chmod +x "$FAKE_BIN"
ASSETS_FAKE="$SANDBOX/assets-fake"
bash "$BUILD_ASSETS" "9.9.9" "$FAKE_BIN" "$ASSETS_FAKE" >/dev/null 2>&1 ||
  _fail "fake upgrade assets" "build-release-assets.sh failed for 9.9.9"

section "installer — argument and policy validation"

run_installer --repository relixiaobo/computer-pilot --allow-unsigned \
  --asset-directory "$ASSETS_REAL" --install-root "$INSTALL_ROOT" --bin-dir "$BIN_DIR"
assert_err_code "missing --version is rejected" "invalid_argument"

run_installer --version "$REAL_VERSION" --repository relixiaobo/computer-pilot \
  --asset-directory "$ASSETS_REAL" --install-root "$INSTALL_ROOT" --bin-dir "$BIN_DIR"
assert_err_code "no --requirement and no --allow-unsigned is rejected" "signing_policy_missing"

run_installer --version "not-a-version" --repository relixiaobo/computer-pilot \
  --allow-unsigned --install-root "$INSTALL_ROOT" --bin-dir "$BIN_DIR"
assert_err_code "malformed version is rejected" "invalid_version"

run_installer --version "$REAL_VERSION" --repository "bad repo name" \
  --allow-unsigned --install-root "$INSTALL_ROOT" --bin-dir "$BIN_DIR"
assert_err_code "malformed repository is rejected" "invalid_repository"

run_installer --version "$REAL_VERSION" --repository relixiaobo/computer-pilot \
  --allow-unsigned --asset-directory "relative/path" \
  --install-root "$INSTALL_ROOT" --bin-dir "$BIN_DIR"
assert_err_code "relative asset directory is rejected" "invalid_install_path"

section "installer — fresh install"

run_installer --version "$REAL_VERSION" --repository relixiaobo/computer-pilot \
  --allow-unsigned --asset-directory "$ASSETS_REAL" \
  --install-root "$INSTALL_ROOT" --bin-dir "$BIN_DIR"
if [[ $EXIT -eq 0 && "$(out_field ok)" == "true" ]]; then
  _pass "fresh install succeeds"
else
  _fail "fresh install succeeds" "exit=$EXIT err=${ERR:0:200}"
fi

if [[ -x "$FIXED" && ! -L "$FIXED" ]]; then
  _pass "binary lives at the fixed realpath (regular file)"
else
  _fail "binary lives at the fixed realpath (regular file)" "missing or symlink: $FIXED"
fi

INSTALLED_VERSION=$("$FIXED" --version 2>/dev/null | awk '{print $2; exit}')
if [[ "$INSTALLED_VERSION" == "$REAL_VERSION" ]]; then
  _pass "installed cu reports v$REAL_VERSION"
else
  _fail "installed cu reports v$REAL_VERSION" "got '$INSTALLED_VERSION'"
fi

if [[ "$(readlink "$BIN_DIR/cu")" == "$FIXED" ]]; then
  _pass "command symlink points at the fixed realpath"
else
  _fail "command symlink points at the fixed realpath" "got '$(readlink "$BIN_DIR/cu" 2>/dev/null)'"
fi

if [[ "$(out_field command)" == "$FIXED" ]]; then
  _pass "output command= is the fixed realpath"
else
  _fail "output command= is the fixed realpath" "got '$(out_field command)'"
fi

if [[ "$(out_field path_ready)" == "false" ]]; then
  _pass "path_ready=false when bin dir is not on PATH"
else
  _fail "path_ready=false when bin dir is not on PATH" "got '$(out_field path_ready)'"
fi

if [[ -f "$INSTALL_ROOT/versions/$REAL_VERSION/cu" ]]; then
  _pass "rollback copy archived under versions/"
else
  _fail "rollback copy archived under versions/" "missing versions/$REAL_VERSION/cu"
fi

section "installer — idempotent re-run"

INODE_BEFORE=$(stat -f %i "$FIXED")
run_installer --version "$REAL_VERSION" --repository relixiaobo/computer-pilot \
  --allow-unsigned --asset-directory "$ASSETS_REAL" \
  --install-root "$INSTALL_ROOT" --bin-dir "$BIN_DIR"
INODE_AFTER=$(stat -f %i "$FIXED")
if [[ $EXIT -eq 0 && "$(out_field ok)" == "true" ]]; then
  _pass "re-run converges with ok=true"
else
  _fail "re-run converges with ok=true" "exit=$EXIT err=${ERR:0:200}"
fi
if [[ "$INODE_BEFORE" == "$INODE_AFTER" ]]; then
  _pass "re-run is a no-op (inode unchanged)"
else
  _fail "re-run is a no-op (inode unchanged)" "$INODE_BEFORE -> $INODE_AFTER"
fi

rm "$BIN_DIR/cu"
run_installer --version "$REAL_VERSION" --repository relixiaobo/computer-pilot \
  --allow-unsigned --asset-directory "$ASSETS_REAL" \
  --install-root "$INSTALL_ROOT" --bin-dir "$BIN_DIR"
if [[ "$(readlink "$BIN_DIR/cu")" == "$FIXED" ]]; then
  _pass "deleted command symlink is restored on re-run"
else
  _fail "deleted command symlink is restored on re-run" "symlink missing after converge"
fi

section "installer — atomic upgrade"

INODE_BEFORE=$(stat -f %i "$FIXED")
run_installer --version "9.9.9" --repository relixiaobo/computer-pilot \
  --allow-unsigned --asset-directory "$ASSETS_FAKE" \
  --install-root "$INSTALL_ROOT" --bin-dir "$BIN_DIR"
INODE_AFTER=$(stat -f %i "$FIXED")
if [[ $EXIT -eq 0 && "$(out_field version)" == "9.9.9" ]]; then
  _pass "upgrade to 9.9.9 succeeds"
else
  _fail "upgrade to 9.9.9 succeeds" "exit=$EXIT err=${ERR:0:200}"
fi
if [[ "$INODE_BEFORE" != "$INODE_AFTER" ]]; then
  _pass "upgrade replaced the binary (inode changed)"
else
  _fail "upgrade replaced the binary (inode changed)" "inode did not change"
fi
UPGRADED_VERSION=$("$FIXED" --version 2>/dev/null | awk '{print $2; exit}')
if [[ "$UPGRADED_VERSION" == "9.9.9" ]]; then
  _pass "fixed realpath now reports 9.9.9"
else
  _fail "fixed realpath now reports 9.9.9" "got '$UPGRADED_VERSION'"
fi
if [[ "$(readlink "$BIN_DIR/cu")" == "$FIXED" ]]; then
  _pass "command symlink target unchanged across upgrade"
else
  _fail "command symlink target unchanged across upgrade" "got '$(readlink "$BIN_DIR/cu" 2>/dev/null)'"
fi

# Roll the sandbox back to the real binary for the remaining cases.
run_installer --version "$REAL_VERSION" --repository relixiaobo/computer-pilot \
  --allow-unsigned --asset-directory "$ASSETS_REAL" \
  --install-root "$INSTALL_ROOT" --bin-dir "$BIN_DIR"

section "installer — corrupted and hostile assets"

ASSETS_CORRUPT="$SANDBOX/assets-corrupt"
cp -R "$ASSETS_REAL" "$ASSETS_CORRUPT"
ARCHIVE_NAME="computer-pilot-v$REAL_VERSION-macos-arm64.tar.gz"
printf 'corruption' >>"$ASSETS_CORRUPT/$ARCHIVE_NAME"
run_installer --version "$REAL_VERSION" --repository relixiaobo/computer-pilot \
  --allow-unsigned --asset-directory "$ASSETS_CORRUPT" \
  --install-root "$SANDBOX/data2" --bin-dir "$SANDBOX/bin2"
assert_err_code "tampered archive is refused" "checksum_mismatch"

ASSETS_BADSIDECAR="$SANDBOX/assets-badsidecar"
cp -R "$ASSETS_REAL" "$ASSETS_BADSIDECAR"
DIGEST=$(awk '{print $1}' "$ASSETS_BADSIDECAR/$ARCHIVE_NAME.sha256")
printf '%s  %s\n' "$DIGEST" "some-other-file.tar.gz" >"$ASSETS_BADSIDECAR/$ARCHIVE_NAME.sha256"
run_installer --version "$REAL_VERSION" --repository relixiaobo/computer-pilot \
  --allow-unsigned --asset-directory "$ASSETS_BADSIDECAR" \
  --install-root "$SANDBOX/data2" --bin-dir "$SANDBOX/bin2"
assert_err_code "sidecar naming a different file is refused" "invalid_checksum"

# Archive smuggling a symbolic link.
ASSETS_SYMLINK="$SANDBOX/assets-symlink"
mkdir -p "$ASSETS_SYMLINK"
SYMROOT="$SANDBOX/symroot/computer-pilot-v7.7.7-macos-arm64"
mkdir -p "$SYMROOT"
cp "$CU" "$SYMROOT/cu"
ln -s /etc/passwd "$SYMROOT/evil"
SYM_ARCHIVE="computer-pilot-v7.7.7-macos-arm64.tar.gz"
tar -C "$SANDBOX/symroot" -czf "$ASSETS_SYMLINK/$SYM_ARCHIVE" "computer-pilot-v7.7.7-macos-arm64"
SYM_SHA=$(shasum -a 256 "$ASSETS_SYMLINK/$SYM_ARCHIVE" | awk '{print $1}')
printf '%s  %s\n' "$SYM_SHA" "$SYM_ARCHIVE" >"$ASSETS_SYMLINK/$SYM_ARCHIVE.sha256"
run_installer --version "7.7.7" --repository relixiaobo/computer-pilot \
  --allow-unsigned --asset-directory "$ASSETS_SYMLINK" \
  --install-root "$SANDBOX/data2" --bin-dir "$SANDBOX/bin2"
assert_err_code "archive containing a symlink is refused" "invalid_archive"

# Archive with an entry outside its root directory.
ASSETS_STRAY="$SANDBOX/assets-stray"
mkdir -p "$ASSETS_STRAY"
STRAYBASE="$SANDBOX/strayroot"
mkdir -p "$STRAYBASE/computer-pilot-v6.6.6-macos-arm64"
cp "$CU" "$STRAYBASE/computer-pilot-v6.6.6-macos-arm64/cu"
printf 'stray' >"$STRAYBASE/stray.txt"
STRAY_ARCHIVE="computer-pilot-v6.6.6-macos-arm64.tar.gz"
tar -C "$STRAYBASE" -czf "$ASSETS_STRAY/$STRAY_ARCHIVE" \
  "computer-pilot-v6.6.6-macos-arm64" "stray.txt"
STRAY_SHA=$(shasum -a 256 "$ASSETS_STRAY/$STRAY_ARCHIVE" | awk '{print $1}')
printf '%s  %s\n' "$STRAY_SHA" "$STRAY_ARCHIVE" >"$ASSETS_STRAY/$STRAY_ARCHIVE.sha256"
run_installer --version "6.6.6" --repository relixiaobo/computer-pilot \
  --allow-unsigned --asset-directory "$ASSETS_STRAY" \
  --install-root "$SANDBOX/data2" --bin-dir "$SANDBOX/bin2"
assert_err_code "archive entry outside its root is refused" "invalid_archive"

# Asset labeled with a version the contained binary does not report.
ASSETS_MISLABELED="$SANDBOX/assets-mislabeled"
bash "$BUILD_ASSETS" "8.8.8" "$CU" "$ASSETS_MISLABELED" >/dev/null 2>&1
run_installer --version "8.8.8" --repository relixiaobo/computer-pilot \
  --allow-unsigned --asset-directory "$ASSETS_MISLABELED" \
  --install-root "$SANDBOX/data2" --bin-dir "$SANDBOX/bin2"
assert_err_code "mislabeled asset version is refused before activation" "version_mismatch"

section "installer — conflicts and shadowing"

CONFLICT_BIN="$SANDBOX/bin-conflict"
mkdir -p "$CONFLICT_BIN"
printf '#!/bin/sh\necho other\n' >"$CONFLICT_BIN/cu"
chmod +x "$CONFLICT_BIN/cu"
run_installer --version "$REAL_VERSION" --repository relixiaobo/computer-pilot \
  --allow-unsigned --asset-directory "$ASSETS_REAL" \
  --install-root "$SANDBOX/data3" --bin-dir "$CONFLICT_BIN"
assert_err_code "unmanaged command at bin path is never replaced" "command_conflict"
if [[ "$(cat "$CONFLICT_BIN/cu")" == *"echo other"* ]]; then
  _pass "unmanaged command content is untouched"
else
  _fail "unmanaged command content is untouched" "file was modified"
fi

DECOY_DIR="$SANDBOX/decoy"
mkdir -p "$DECOY_DIR"
printf '#!/bin/sh\necho decoy\n' >"$DECOY_DIR/cu"
chmod +x "$DECOY_DIR/cu"
EXIT=0
OUT=$(env PATH="$DECOY_DIR:$RESTRICTED_PATH" HOME="$SANDBOX/home" \
  sh "$INSTALLER" --version "$REAL_VERSION" --repository relixiaobo/computer-pilot \
  --allow-unsigned --asset-directory "$ASSETS_REAL" \
  --install-root "$INSTALL_ROOT" --bin-dir "$BIN_DIR" 2>/tmp/cu-test-stderr) || EXIT=$?
if [[ $EXIT -eq 0 && "$(out_field shadowed_by)" == "$DECOY_DIR/cu" ]]; then
  _pass "another cu earlier on PATH is reported as shadowed_by"
else
  _fail "another cu earlier on PATH is reported as shadowed_by" "exit=$EXIT shadowed_by='$(out_field shadowed_by)'"
fi

summary
