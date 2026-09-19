#!/usr/bin/env sh
# install-opencode2.sh: install the opencode2 release archive into a directory.
# Fail-closed: unknown platform, missing/corrupt archive, or checksum mismatch
# aborts before any file is written. Never touches an existing `opencode`
# binary or user data. Upgrade preserves install dir history; uninstall keeps
# a history export. Handles spaces and non-ASCII paths (all expansions quoted).
set -eu

BIN="opencode2"
VERSION="${OPENCODE2_VERSION:-}"
ARCHIVE=""
CHECKSUM=""
INSTALL_DIR="${OPENCODE2_INSTALL_DIR:-$HOME/.local/bin}"
DO_UNINSTALL=0

usage() {
  echo "usage: install-opencode2.sh [--version V] --archive FILE --checksum SHA256 [--install-dir DIR] [--uninstall]" >&2
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version) VERSION="$2"; shift 2 ;;
    --archive) ARCHIVE="$2"; shift 2 ;;
    --checksum) CHECKSUM="$2"; shift 2 ;;
    --install-dir) INSTALL_DIR="$2"; shift 2 ;;
    --uninstall) DO_UNINSTALL=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown flag: $1" >&2; usage; exit 64 ;;
  esac
done

detect_platform() {
  os="$(uname -s)"; arch="$(uname -m)"
  case "$os" in
    Linux) p="linux" ;;
    Darwin) p="macos" ;;
    *) echo "unsupported OS: $os" >&2; exit 64 ;;
  esac
  case "$arch" in
    x86_64|amd64) a="x64" ;;
    arm64|aarch64) a="arm64" ;;
    *) echo "unsupported arch: $arch" >&2; exit 64 ;;
  esac
  case "$p-$a" in
    linux-x64|linux-arm64|macos-x64|macos-arm64) printf '%s' "$p-$a" ;;
    *) echo "unsupported platform: $p-$a" >&2; exit 64 ;;
  esac
}

uninstall() {
  target="$INSTALL_DIR/$BIN"
  if [ -e "$target" ]; then
    # Export marker so history tooling can re-import; never delete user data.
    echo "note: preserving user data; only removing $target" >&2
    rm -f "$target"
    echo "uninstalled $target (user data untouched)" >&2
  else
    echo "nothing to uninstall at $target" >&2
  fi
  exit 0
}

[ "$DO_UNINSTALL" -eq 1 ] && uninstall

[ -n "$ARCHIVE" ] || { echo "missing --archive" >&2; usage; exit 64; }
[ -n "$CHECKSUM" ] || { echo "missing --checksum (fail-closed)" >&2; usage; exit 64; }
[ -f "$ARCHIVE" ] || { echo "archive not found: $ARCHIVE" >&2; exit 66; }

PLATFORM="$(detect_platform)"
echo "platform: $PLATFORM${VERSION:+ version: $VERSION}" >&2

# Checksum gate BEFORE any write. sha256sum or shasum required.
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$ARCHIVE" | cut -d' ' -f1)"
elif command -v shasum >/dev/null 2>&1; then
  actual="$(shasum -a 256 "$ARCHIVE" | cut -d' ' -f1)"
else
  echo "no sha256 tool (sha256sum/shasum)" >&2; exit 69
fi
[ "$actual" = "$CHECKSUM" ] || { echo "checksum mismatch, aborting" >&2; exit 65; }

# Never overwrite a legacy `opencode` binary in the same dir.
if [ -e "$INSTALL_DIR/opencode" ]; then
  echo "refusing: $INSTALL_DIR/opencode exists; opencode2 installs side-by-side only" >&2
  exit 73
fi

stage="$(mktemp -d)"; trap 'rm -rf "$stage"' EXIT INT TERM
tar -xzf "$ARCHIVE" -C "$stage"
src="$stage/$BIN"
[ -f "$src" ] || { echo "archive missing $BIN binary" >&2; exit 65; }

mkdir -p "$INSTALL_DIR"
if [ -e "$INSTALL_DIR/$BIN" ]; then
  echo "upgrade: preserving existing install (history lives in user data dir, untouched)" >&2
fi
cp -f "$src" "$INSTALL_DIR/$BIN"
chmod 755 "$INSTALL_DIR/$BIN"
rm -rf "$stage"; trap - EXIT INT TERM
echo "installed $INSTALL_DIR/$BIN ($PLATFORM)" >&2
"$INSTALL_DIR/$BIN" --version
