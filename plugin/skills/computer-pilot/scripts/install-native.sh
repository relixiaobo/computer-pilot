#!/bin/sh
# Computer Pilot native installer.
#
# Converges the local machine to the release pinned by the skill's
# compatibility.json: download (or read --asset-directory), verify the SHA-256
# sidecar, verify the Developer ID codesign requirement when one is declared,
# then atomically activate the binary at a FIXED realpath so macOS TCC
# permissions survive upgrades. Never uses sudo, never touches
# /usr/local/bin, never resolves a `latest` URL.
#
# Layout:
#   <install-root>/bin/cu           fixed realpath (atomic rename on upgrade)
#   <install-root>/versions/<v>/cu  archived copies for manual rollback
#   <bin-dir>/cu                    stable symlink -> <install-root>/bin/cu

set -eu
set -f

UNSUPPORTED_PLATFORM_EXIT_CODE=10

# Keep in sync with installation.asset_template in compatibility.json and with
# scripts/build-release-assets.sh; tests/commands/test_release_contract.sh
# asserts all three agree. Callers that have the manifest (the skill preflight,
# any Agent host) should pass --asset-template rather than rely on this default.
DEFAULT_ASSET_TEMPLATE='computer-pilot-v{version}-macos-arm64.tar.gz'

# Matches installation.signing.identifier in compatibility.json (also asserted
# by tests/commands/test_release_contract.sh). Used only to recognize an older
# Computer Pilot occupying the command path — never as a trust decision.
SIGNING_IDENTIFIER='com.linlab.computer-pilot.cu'

version=''
repository=''
requirement=''
asset_template=''
allow_unsigned=false
asset_directory=''
install_root=${COMPUTER_PILOT_INSTALL_ROOT:-}
bin_directory=${COMPUTER_PILOT_BIN_DIR:-}
temporary_directory=''
staging_binary=''
temporary_link=''

usage() {
  printf '%s\n' \
    'Usage: sh install-native.sh --version <version> --repository <owner/repo>' \
    '       [--requirement <codesign requirement>] [--allow-unsigned]' \
    '       [--asset-template <name with {version}>]' \
    '       [--asset-directory <path>] [--install-root <path>] [--bin-dir <path>]'
}

fail() {
  code=$1
  shift
  printf 'code=%s\nmessage=%s\n' "$code" "$*" >&2
  exit 1
}

cleanup() {
  if [ -n "$temporary_link" ] && { [ -e "$temporary_link" ] || [ -L "$temporary_link" ]; }; then
    rm -f -- "$temporary_link"
  fi
  if [ -n "$staging_binary" ] && [ -e "$staging_binary" ]; then
    rm -f -- "$staging_binary"
  fi
  if [ -n "$temporary_directory" ] && [ -d "$temporary_directory" ]; then
    rm -rf -- "$temporary_directory"
  fi
}

trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      [ "$#" -ge 2 ] || { usage >&2; exit 2; }
      version=$2
      shift 2
      ;;
    --repository)
      [ "$#" -ge 2 ] || { usage >&2; exit 2; }
      repository=$2
      shift 2
      ;;
    --requirement)
      [ "$#" -ge 2 ] || { usage >&2; exit 2; }
      requirement=$2
      shift 2
      ;;
    --asset-template)
      [ "$#" -ge 2 ] || { usage >&2; exit 2; }
      asset_template=$2
      shift 2
      ;;
    --allow-unsigned)
      allow_unsigned=true
      shift
      ;;
    --asset-directory)
      [ "$#" -ge 2 ] || { usage >&2; exit 2; }
      asset_directory=$2
      shift 2
      ;;
    --install-root)
      [ "$#" -ge 2 ] || { usage >&2; exit 2; }
      install_root=$2
      shift 2
      ;;
    --bin-dir)
      [ "$#" -ge 2 ] || { usage >&2; exit 2; }
      bin_directory=$2
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      usage >&2
      fail invalid_argument "Unknown argument: $1"
      ;;
  esac
done

[ -n "$version" ] || { usage >&2; fail invalid_argument '--version is required'; }
[ -n "$repository" ] || { usage >&2; fail invalid_argument '--repository is required'; }
printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$' ||
  fail invalid_version "Invalid release version: $version"
printf '%s\n' "$repository" | grep -Eq '^[0-9A-Za-z_.-]+/[0-9A-Za-z_.-]+$' ||
  fail invalid_repository "Invalid GitHub repository: $repository"
[ -n "$asset_template" ] || asset_template=$DEFAULT_ASSET_TEMPLATE
case "$asset_template" in
  *'{version}'*) ;;
  *) fail invalid_asset_template "Asset template must contain {version}: $asset_template" ;;
esac
case "$asset_template" in
  */*|*..*) fail invalid_asset_template "Asset template must be a plain file name: $asset_template" ;;
esac
if [ -z "$requirement" ] && [ "$allow_unsigned" != 'true' ]; then
  fail signing_policy_missing \
    'Provide --requirement from compatibility.json installation.signing.requirement, or pass --allow-unsigned only when that manifest declares required_status=ad-hoc-unsigned'
fi

# Computer Pilot supports Apple Silicon macOS only. A Rosetta shell reports
# x86_64 while hw.optional.arm64=1; the native binary still applies.
system_name=$(uname -s 2>/dev/null || true)
machine_name=$(uname -m 2>/dev/null || true)
if [ "$system_name" != 'Darwin' ]; then
  printf 'code=unsupported_platform\nplatform=%s\narchitecture=%s\n' \
    "$system_name" "$machine_name" >&2
  exit "$UNSUPPORTED_PLATFORM_EXIT_CODE"
fi
if [ "$machine_name" != 'arm64' ] &&
  { [ "$machine_name" != 'x86_64' ] ||
    [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" != '1' ]; }; then
  printf 'code=unsupported_platform\nplatform=%s\narchitecture=%s\n' \
    "$system_name" "$machine_name" >&2
  exit "$UNSUPPORTED_PLATFORM_EXIT_CODE"
fi

if [ -z "$install_root" ]; then
  [ -n "${HOME:-}" ] || fail missing_home 'HOME is required unless --install-root is provided'
  install_root=${XDG_DATA_HOME:-"$HOME/.local/share"}/computer-pilot
fi

path_contains() {
  candidate=$1
  case ":${PATH:-}:" in
    *":$candidate:"*) return 0 ;;
    *) return 1 ;;
  esac
}

if [ -z "$bin_directory" ]; then
  [ -n "${HOME:-}" ] || fail missing_home 'HOME is required unless --bin-dir is provided'
  if path_contains "$HOME/.local/bin"; then
    bin_directory=$HOME/.local/bin
  elif path_contains "$HOME/bin"; then
    bin_directory=$HOME/bin
  else
    bin_directory=$HOME/.local/bin
  fi
fi

case "$install_root" in
  /*) ;;
  *) fail invalid_install_path "Installation root must be absolute: $install_root" ;;
esac
[ "$install_root" != '/' ] || fail invalid_install_path 'Installation root cannot be the filesystem root'
case "$bin_directory" in
  /*) ;;
  *) fail invalid_install_path "Command directory must be absolute: $bin_directory" ;;
esac
[ "$bin_directory" != '/' ] || fail invalid_install_path 'Command directory cannot be the filesystem root'
if [ -n "$asset_directory" ]; then
  case "$asset_directory" in
    /*) ;;
    *) fail invalid_install_path "Asset directory must be absolute: $asset_directory" ;;
  esac
fi

fixed_binary=$install_root/bin/cu

verify_requirement() {
  candidate=$1
  if [ -n "$requirement" ]; then
    command -v codesign >/dev/null 2>&1 ||
      fail codesign_missing 'codesign is required to verify the release signature'
    codesign --verify --strict -R="$requirement" "$candidate" >/dev/null 2>&1 ||
      return 1
  fi
  return 0
}

binary_version() {
  "$1" --version 2>/dev/null | awk '{print $2; exit}'
}

emit_result() {
  installed_command=$1
  # path_ready answers the only question callers actually have: does a plain
  # `cu` reach Computer Pilot? macOS ships an unrelated /usr/bin/cu (UUCP
  # dialer) that wins on the stock PATH, so "our bin dir is somewhere on
  # PATH" is not sufficient — resolve the command and compare.
  resolved=$(command -v cu 2>/dev/null || true)
  shadowed_by=''
  if [ "$resolved" = "$installed_command" ] || [ "$resolved" = "$bin_directory/cu" ]; then
    path_ready=true
  else
    path_ready=false
    [ -n "$resolved" ] && shadowed_by=$resolved
  fi
  printf 'ok=true\nversion=%s\ncommand=%s\ninstall_bin_dir=%s\nbin_directory=%s\npath_ready=%s\n' \
    "$version" "$installed_command" "$install_root/bin" "$bin_directory" "$path_ready"
  if [ -n "$shadowed_by" ]; then
    printf 'shadowed_by=%s\n' "$shadowed_by"
  fi
  if [ "$path_ready" != 'true' ]; then
    printf 'path_hint=prepend %s to PATH, or invoke %s directly\n' \
      "$install_root/bin" "$installed_command"
  fi
}

is_owned_link() {
  link_path=$1
  if [ ! -e "$link_path" ] && [ ! -L "$link_path" ]; then
    return 0
  fi
  [ -L "$link_path" ] || return 1
  link_target=$(readlink "$link_path") || return 1
  [ "$link_target" = "$fixed_binary" ]
}

# Say what is already sitting at a command path, so the refusal names a way
# out instead of a dead end. Read the code signature rather than running the
# file: it is unknown by definition, and executing it to ask its version
# would be the one thing this refusal exists to avoid.
describe_unmanaged_command() {
  conflict_path=$1
  conflict_identifier=$(codesign -dvv "$conflict_path" 2>&1 | sed -n 's/^Identifier=//p')
  case "$conflict_identifier" in
    "$SIGNING_IDENTIFIER")
      printf 'an older Computer Pilot installed outside the managed layout — remove it and re-run' ;;
    com.apple.*)
      printf 'an Apple system binary (%s) — pass --bin-dir to install elsewhere' "$conflict_identifier" ;;
    '')
      printf 'unidentified — remove it and re-run, or pass --bin-dir to install elsewhere' ;;
    *)
      printf 'code identifier %s — remove it and re-run, or pass --bin-dir to install elsewhere' \
        "$conflict_identifier" ;;
  esac
}

refuse_unmanaged_command() {
  conflict=$1
  fail command_conflict \
    "Refusing to replace an unmanaged command: $conflict ($(describe_unmanaged_command "$conflict"))"
}

ensure_command_link() {
  mkdir -p "$bin_directory" ||
    fail install_directory_failed "Could not create $bin_directory"
  is_owned_link "$bin_directory/cu" || refuse_unmanaged_command "$bin_directory/cu"
  if [ ! -L "$bin_directory/cu" ]; then
    temporary_link=$bin_directory/.cu.$$.tmp
    [ ! -e "$temporary_link" ] && [ ! -L "$temporary_link" ] ||
      fail command_conflict "Temporary command path already exists: $temporary_link"
    ln -s "$fixed_binary" "$temporary_link" ||
      fail install_failed 'Could not create the cu command'
    mv -f "$temporary_link" "$bin_directory/cu" ||
      fail install_failed 'Could not activate the cu command'
    temporary_link=''
  fi
}

# Idempotent fast path: the fixed binary already reports the requested version
# and satisfies the declared signing requirement. Still converge the command
# symlink so a deleted link is restored.
if [ -x "$fixed_binary" ] && [ ! -L "$fixed_binary" ] &&
  [ "$(binary_version "$fixed_binary")" = "$version" ] &&
  verify_requirement "$fixed_binary"; then
  ensure_command_link
  emit_result "$fixed_binary"
  exit 0
fi

archive_name=$(printf '%s' "$asset_template" | sed "s/{version}/$version/g")
archive_root=${archive_name%.tar.gz}
[ "$archive_root" != "$archive_name" ] ||
  fail invalid_asset_template "Asset template must name a .tar.gz archive: $asset_template"
temporary_directory=$(mktemp -d "${TMPDIR:-/tmp}/computer-pilot-install.XXXXXX") ||
  fail temporary_directory_failed 'Could not create a temporary directory'

if [ -n "$asset_directory" ]; then
  [ -d "$asset_directory" ] || fail asset_directory_missing "Asset directory does not exist: $asset_directory"
  archive_path=$asset_directory/$archive_name
  checksum_path=$asset_directory/$archive_name.sha256
else
  archive_path=$temporary_directory/$archive_name
  checksum_path=$temporary_directory/$archive_name.sha256
  release_url="https://github.com/$repository/releases/download/v$version"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL --retry 2 --output "$archive_path" "$release_url/$archive_name" ||
      fail download_failed "Could not download $archive_name"
    curl -fsSL --retry 2 --output "$checksum_path" "$release_url/$archive_name.sha256" ||
      fail download_failed "Could not download $archive_name.sha256"
  elif command -v wget >/dev/null 2>&1; then
    wget -q -O "$archive_path" "$release_url/$archive_name" ||
      fail download_failed "Could not download $archive_name"
    wget -q -O "$checksum_path" "$release_url/$archive_name.sha256" ||
      fail download_failed "Could not download $archive_name.sha256"
  else
    fail downloader_missing 'curl or wget is required to download the native release'
  fi
fi

[ -f "$archive_path" ] || fail archive_missing "Release archive is missing: $archive_path"
[ -f "$checksum_path" ] || fail checksum_missing "Checksum sidecar is missing: $checksum_path"

checksum_line=$(sed -n '1p' "$checksum_path")
set -- $checksum_line
[ "$#" -eq 2 ] || fail invalid_checksum "Invalid checksum sidecar: $checksum_path"
expected_checksum=$1
checksum_file_name=$2
printf '%s\n' "$expected_checksum" | grep -Eq '^[0-9a-fA-F]{64}$' ||
  fail invalid_checksum "Invalid SHA-256 value in $checksum_path"
[ "$checksum_file_name" = "$archive_name" ] ||
  fail invalid_checksum "Checksum sidecar names $checksum_file_name instead of $archive_name"

if command -v sha256sum >/dev/null 2>&1; then
  actual_checksum=$(sha256sum "$archive_path" | awk '{print $1}')
elif command -v shasum >/dev/null 2>&1; then
  actual_checksum=$(shasum -a 256 "$archive_path" | awk '{print $1}')
else
  fail checksum_tool_missing 'sha256sum or shasum is required to verify the native release'
fi

[ "$actual_checksum" = "$expected_checksum" ] ||
  fail checksum_mismatch "SHA-256 verification failed for $archive_name"

command -v tar >/dev/null 2>&1 || fail extractor_missing 'tar is required to extract the native release'
archive_listing=$temporary_directory/archive-list.txt
tar -tzf "$archive_path" > "$archive_listing" ||
  fail invalid_archive "Could not inspect $archive_name"

[ -s "$archive_listing" ] || fail invalid_archive "Archive is empty: $archive_name"
while IFS= read -r archive_entry || [ -n "$archive_entry" ]; do
  case "$archive_entry" in
    "$archive_root"|"$archive_root"/*) ;;
    *) fail invalid_archive "Archive entry is outside $archive_root: $archive_entry" ;;
  esac
  case "/$archive_entry/" in
    *'/../'*|*'/./'*) fail invalid_archive "Archive entry is unsafe: $archive_entry" ;;
  esac
done < "$archive_listing"

mkdir -p "$temporary_directory/extracted"
tar -xzf "$archive_path" -C "$temporary_directory/extracted" ||
  fail extraction_failed "Could not extract $archive_name"

source_directory=$temporary_directory/extracted/$archive_root
source_executable=$source_directory/cu
[ -d "$source_directory" ] || fail invalid_archive "Archive root is missing: $archive_root"
[ -f "$source_executable" ] && [ ! -L "$source_executable" ] ||
  fail invalid_archive 'Archive does not contain a regular cu executable'
if find "$source_directory" -type l -print -quit | grep -q .; then
  fail invalid_archive 'Archive contains an unexpected symbolic link'
fi
chmod 0755 "$source_executable" ||
  fail install_failed 'Could not make the cu executable runnable'

verify_requirement "$source_executable" ||
  fail signature_mismatch "cu $version does not satisfy the declared codesign requirement"
staged_version=$(binary_version "$source_executable")
[ "$staged_version" = "$version" ] ||
  fail version_mismatch "Release asset reports cu version '$staged_version' instead of '$version'"

versions_directory=$install_root/versions
mkdir -p "$install_root/bin" "$versions_directory" "$bin_directory" ||
  fail install_directory_failed 'Could not create the installation directories'

if [ -L "$fixed_binary" ]; then
  fail install_conflict "Fixed binary path is a symlink, refusing to replace: $fixed_binary"
fi

is_owned_link "$bin_directory/cu" || refuse_unmanaged_command "$bin_directory/cu"

# Keep an archived copy for manual rollback. Never activated in place; the
# fixed realpath below is the only executable path agents and TCC ever see.
archived_copy=$versions_directory/$version/cu
if [ ! -f "$archived_copy" ]; then
  mkdir -p "$versions_directory/$version" ||
    fail install_directory_failed "Could not create $versions_directory/$version"
  cp "$source_executable" "$archived_copy.tmp.$$" &&
    mv "$archived_copy.tmp.$$" "$archived_copy" ||
    fail install_failed "Could not archive cu $version"
fi

# Atomic activation: stage on the same volume, then rename over the fixed
# path. Never write into a running binary in place (macOS kills processes
# whose backing code file is mutated); rename preserves both the old
# process's inode and the fixed TCC-visible path.
staging_binary=$install_root/bin/.cu.install.$$
[ ! -e "$staging_binary" ] || fail install_conflict "Staging path already exists: $staging_binary"
cp "$source_executable" "$staging_binary" ||
  fail install_failed "Could not stage cu in $install_root/bin"
chmod 0755 "$staging_binary" ||
  fail install_failed 'Could not make the staged cu runnable'
mv -f "$staging_binary" "$fixed_binary" ||
  fail install_failed "Could not activate cu at $fixed_binary"
staging_binary=''

ensure_command_link
emit_result "$fixed_binary"
