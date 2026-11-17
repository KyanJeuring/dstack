#!/bin/sh

set -eu

repository="KyanJeuring/dstack"
version_input=${DSTACK_VERSION-}
install_dir=${DSTACK_INSTALL_DIR-}
dry_run=false

usage() {
  cat <<'EOF'
Usage: install-dstack.sh [--version v2.X.Y] [--install-dir PATH] [--dry-run]

Environment equivalents:
  DSTACK_VERSION       Stable v2 release, with or without the leading v
  DSTACK_INSTALL_DIR   Destination directory (default: $HOME/.local/bin)
EOF
}

fail() {
  printf 'install-dstack: %s\n' "$*" >&2
  exit 1
}

while [ "$#" -gt 0 ]; do
  case $1 in
    --version)
      [ "$#" -ge 2 ] || fail "--version requires a value"
      version_input=$2
      shift 2
      ;;
    --install-dir)
      [ "$#" -ge 2 ] || fail "--install-dir requires a value"
      install_dir=$2
      shift 2
      ;;
    --dry-run)
      dry_run=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      fail "unknown option: $1"
      ;;
  esac
done

if [ -z "$install_dir" ]; then
  [ -n "${HOME-}" ] || fail "HOME is not set; use --install-dir or DSTACK_INSTALL_DIR"
  install_dir=$HOME/.local/bin
fi

normalize_version() (
  value=${1#v}
  major=${value%%.*}
  remainder=${value#*.}
  [ "$remainder" != "$value" ] || exit 1
  minor=${remainder%%.*}
  patch=${remainder#*.}
  [ "$patch" != "$remainder" ] || exit 1
  case $patch in
    *.*) exit 1 ;;
  esac
  for component in "$major" "$minor" "$patch"; do
    case $component in
      ''|*[!0-9]*) exit 1 ;;
    esac
  done
  [ "$major" -eq 2 ] 2>/dev/null || exit 1
  printf 'v%s.%s.%s\n' "$major" "$minor" "$patch"
)

resolve_latest_version() {
  command -v curl >/dev/null 2>&1 || fail "curl is required to resolve the latest release"
  latest_url=$(curl -fsSL -o /dev/null -w '%{url_effective}' \
    "https://github.com/$repository/releases/latest") ||
    fail "could not resolve the latest published GitHub release"
  latest_tag=${latest_url##*/}
  normalize_version "$latest_tag" ||
    fail "the latest published GitHub release is not a stable v2.X.Y release"
}

if [ -n "$version_input" ]; then
  version=$(normalize_version "$version_input") ||
    fail "version must be a stable v2.X.Y release"
else
  version=$(resolve_latest_version)
fi

case $(uname -s) in
  Linux) os=linux ;;
  Darwin) os=macos ;;
  *) fail "unsupported operating system: $(uname -s)" ;;
esac

case $(uname -m) in
  x86_64|amd64) arch=x86_64 ;;
  aarch64|arm64) arch=aarch64 ;;
  *) fail "unsupported architecture: $(uname -m)" ;;
esac

platform=$os-$arch
archive=dstack-$version-$platform.tar.gz
destination=$install_dir/dstack

printf 'DStack installer plan\n'
printf '  Version: %s\n' "$version"
printf '  Platform: %s\n' "$platform"
printf '  Release asset: %s\n' "$archive"
printf '  Destination: %s\n' "$destination"

if [ "$dry_run" = true ]; then
  printf 'Dry run: no network request or filesystem change was made.\n'
  exit 0
fi

command -v curl >/dev/null 2>&1 || fail "curl is required to download DStack"
command -v tar >/dev/null 2>&1 || fail "tar is required to extract DStack"

if command -v sha256sum >/dev/null 2>&1; then
  checksum_tool=sha256sum
elif command -v shasum >/dev/null 2>&1; then
  checksum_tool=shasum
else
  fail "SHA-256 verification requires sha256sum or shasum"
fi

temp_root=$(mktemp -d "${TMPDIR:-/tmp}/install-dstack.XXXXXX") ||
  fail "could not create a temporary directory"
stage_path=
cleanup() {
  if [ -n "$stage_path" ] && [ -e "$stage_path" ]; then
    rm -f "$stage_path"
  fi
  rm -rf "$temp_root"
}
trap cleanup EXIT HUP INT TERM

archive_path=$temp_root/$archive
checksum_path=$temp_root/$archive.sha256
release_base=https://github.com/$repository/releases/download/$version

curl -fsSL "$release_base/$archive" -o "$archive_path" ||
  fail "could not download $archive"
curl -fsSL "$release_base/$archive.sha256" -o "$checksum_path" ||
  fail "could not download $archive.sha256; installation requires its checksum"

checksum_line=$(sed -n '1p' "$checksum_path")
awk 'NR > 1 { exit 1 } END { if (NR != 1) exit 1 }' "$checksum_path" ||
  fail "checksum sidecar must contain exactly one checksum line"
[ -n "$checksum_line" ] || fail "checksum sidecar is malformed"
expected_checksum=${checksum_line%%  *}
checksum_filename=${checksum_line#*  }
[ "$checksum_line" = "$expected_checksum  $checksum_filename" ] ||
  fail "checksum sidecar is malformed"
case $expected_checksum in
  *[!0-9a-fA-F]*|'') fail "checksum sidecar has an invalid SHA-256 digest" ;;
esac
[ "${#expected_checksum}" -eq 64 ] || fail "checksum sidecar has an invalid SHA-256 digest"
[ "$checksum_filename" = "$archive" ] || fail "checksum sidecar names the wrong release asset"

if [ "$checksum_tool" = sha256sum ]; then
  actual_checksum=$(sha256sum "$archive_path")
else
  actual_checksum=$(shasum -a 256 "$archive_path")
fi
actual_checksum=${actual_checksum%% *}
[ "$actual_checksum" = "$expected_checksum" ] || fail "SHA-256 checksum mismatch for $archive"

archive_entries=$(tar -tzf "$archive_path") || fail "could not inspect $archive"
[ "$archive_entries" = dstack ] || fail "release archive must contain only the dstack executable"

extract_dir=$temp_root/extracted
mkdir "$extract_dir"
tar -xzf "$archive_path" -C "$extract_dir" || fail "could not extract $archive"
staged_binary=$extract_dir/dstack
if [ ! -f "$staged_binary" ] || [ -L "$staged_binary" ]; then
  fail "release archive does not contain the expected dstack executable"
fi
chmod 755 "$staged_binary"

expected_output="dstack ${version#v}"
actual_output=$("$staged_binary" --version) || fail "staged DStack executable failed its version check"
[ "$actual_output" = "$expected_output" ] ||
  fail "staged executable reported '$actual_output'; expected '$expected_output'"

mkdir -p "$install_dir"
stage_path=$(mktemp "$install_dir/.dstack.install.XXXXXX") ||
  fail "could not stage DStack beside $destination"
cp "$staged_binary" "$stage_path"
chmod 755 "$stage_path"
mv -f "$stage_path" "$destination" || fail "could not replace $destination"
stage_path=

printf 'Installed %s to %s\n' "$expected_output" "$destination"
case :${PATH-}: in
  *:"$install_dir":*) ;;
  *)
    printf 'The install directory is not on PATH. Add this exact directory: %s\n' "$install_dir"
    ;;
esac
printf 'After dstack is on PATH, run: dstack shell install\n'

if [ -n "${HOME-}" ] && [ -f "$HOME/.local/share/dstack/dstack.sh" ]; then
  printf 'DStack v2 was installed to %s.\n' "$destination"
  printf 'The legacy v1 Bash integration may still shadow the v2 binary.\n'
  printf 'Follow MIGRATING_V1_TO_V2.md before expecting `dstack` to invoke v2:\n'
  printf '  https://github.com/%s/blob/main/MIGRATING_V1_TO_V2.md\n' "$repository"
  printf 'Preview registry migration with the installed v2 executable:\n'
  printf '  "%s" migrate registry --dry-run\n' "$destination"
  printf 'Then migrate after reviewing the preview:\n'
  printf '  "%s" migrate registry\n' "$destination"
  printf 'Legacy v1 files and shell-profile lines were left unchanged.\n'
fi
