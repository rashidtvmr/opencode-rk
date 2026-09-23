#!/usr/bin/env sh
# install-oc2.sh: install the oc2 release archive into a directory.
# Fail-closed: unknown platform, missing/corrupt archive, or checksum mismatch
# aborts before any file is written. Never touches an existing `opencode`
# binary or user data. Upgrade preserves install dir history; uninstall keeps
# a history export. Handles spaces and non-ASCII paths (all expansions quoted).
set -eu

BIN="oc2"
VERSION="${OC2_VERSION:-}"
ARCHIVE=""
CHECKSUM=""
INSTALL_DIR="${OC2_INSTALL_DIR:-$HOME/.local/bin}"
DO_UNINSTALL=0
MAX_ARCHIVE_MEMBERS=4
# Fixed release closure bound. Each regular member and the combined expanded
# payload are capped independently; neither value is environment-configurable.
MAX_ARCHIVE_MEMBER_PAYLOAD=134217728
MAX_ARCHIVE_PAYLOAD=134217728
MAX_ARCHIVE_PROBE_BYTES=134217729

# Normalize an unsigned decimal without arithmetic. This keeps leading-zero
# test/tool output from changing shell integer interpretation.
normalize_uint() {
  case "$1" in
    ''|*[!0-9]*) return 1 ;;
  esac
  NORMALIZED_UINT="$1"
  while [ "$NORMALIZED_UINT" != "0" ] && [ "${NORMALIZED_UINT#0}" != "$NORMALIZED_UINT" ]; do
    NORMALIZED_UINT="${NORMALIZED_UINT#0}"
  done
  return 0
}

# Compare unsigned decimals without arithmetic on attacker-sized operands.
# Equal-length values are compared as strings; length handles every larger
# value before any shell integer operation.
uint_gt() {
  _uint_left="$1"
  _uint_right="$2"
  normalize_uint "$_uint_left" || return 2
  _uint_left="$NORMALIZED_UINT"
  normalize_uint "$_uint_right" || return 2
  _uint_right="$NORMALIZED_UINT"
  _uint_left_len=${#_uint_left}
  _uint_right_len=${#_uint_right}
  if [ "$_uint_left_len" -gt "$_uint_right_len" ]; then
    return 0
  fi
  if [ "$_uint_left_len" -lt "$_uint_right_len" ]; then
    return 1
  fi
  # Prefix keeps awk from treating long digit strings as floating-point values.
  LC_ALL=C awk -v left="x$_uint_left" -v right="x$_uint_right" 'BEGIN { exit !(left > right) }'
}

usage() {
  echo "usage: install-oc2.sh [--version V] --archive FILE --checksum SHA256 [--install-dir DIR] [--uninstall]" >&2
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

stage=""
transaction_active=0
target=""
native_target=""
binary_tmp=""
native_tmp=""
binary_backup=""
native_backup=""
binary_backed=0
native_backed=0
binary_installed=0
native_installed=0

cleanup() {
  if [ -n "$stage" ]; then
    rm -rf "$stage" 2>/dev/null || :
  fi
  rm -f "$binary_tmp" "$native_tmp" 2>/dev/null || :
  if [ "$binary_backed" -eq 0 ]; then
    rm -f "$binary_backup" 2>/dev/null || :
  fi
  if [ "$native_backed" -eq 0 ]; then
    rm -f "$native_backup" 2>/dev/null || :
  fi
}

rollback_install() {
  rollback_code="$1"
  transaction_active=0

  if [ "$native_installed" -eq 1 ]; then
    rm -f "$native_target" 2>/dev/null || :
  fi
  if [ "$binary_installed" -eq 1 ]; then
    rm -f "$target" 2>/dev/null || :
  fi
  if [ "$native_backed" -eq 1 ]; then
    if ! mv "$native_backup" "$native_target" 2>/dev/null; then
      echo "FAIL: could not restore previous native library" >&2
    fi
  fi
  if [ "$binary_backed" -eq 1 ]; then
    if ! mv "$binary_backup" "$target" 2>/dev/null; then
      echo "FAIL: could not restore previous binary" >&2
    fi
  fi
  rm -f "$binary_tmp" "$native_tmp" 2>/dev/null || :
  echo "FAIL: install transaction rolled back" >&2
  exit "$rollback_code"
}

on_signal() {
  signal_code="$1"
  if [ "$transaction_active" -eq 1 ]; then
    rollback_install "$signal_code"
  fi
  exit "$signal_code"
}

trap cleanup 0
trap 'on_signal 130' 2
trap 'on_signal 143' 15

uninstall() {
  target="$INSTALL_DIR/$BIN"
  native_dir="$INSTALL_DIR/../lib"
  native_target="$native_dir/$NATIVE_NAME"
  if [ -L "$INSTALL_DIR" ] || [ -L "$native_dir" ]; then
    echo "refusing: destination directory is a symlink" >&2
    exit 74
  fi
  if [ -e "$target" ] || [ -L "$target" ]; then
    # Export marker so history tooling can re-import; never delete user data.
    echo "note: preserving user data; only removing $target" >&2
    rm -f "$target"
    echo "uninstalled $target (user data untouched)" >&2
  else
    echo "nothing to uninstall at $target" >&2
  fi
  if [ -e "$native_target" ] || [ -L "$native_target" ]; then
    rm -f "$native_target"
    echo "uninstalled $native_target (user data untouched)" >&2
  fi
  exit 0
}

if [ "$DO_UNINSTALL" -eq 0 ]; then
  [ -n "$ARCHIVE" ] || { echo "missing --archive" >&2; usage; exit 64; }
  [ -n "$CHECKSUM" ] || { echo "missing --checksum (fail-closed)" >&2; usage; exit 64; }
  [ -f "$ARCHIVE" ] || { echo "archive not found: $ARCHIVE" >&2; exit 66; }
fi

PLATFORM="$(detect_platform)"
echo "platform: $PLATFORM${VERSION:+ version: $VERSION}" >&2
case "$PLATFORM" in
  linux-*) LIB_SUFFIX=".so" ;;
  macos-*) LIB_SUFFIX=".dylib" ;;
  *) echo "unsupported platform: $PLATFORM" >&2; exit 64 ;;
esac
NATIVE_NAME="libopentui$LIB_SUFFIX"
EXPECTED_NATIVE="native/lib/$PLATFORM/$NATIVE_NAME"
[ "$DO_UNINSTALL" -eq 1 ] && uninstall

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
[ ! -e "$INSTALL_DIR/opencode" ] || {
  echo "refusing: $INSTALL_DIR/opencode exists; oc2 installs side-by-side only" >&2
  exit 73
}

stage="$(mktemp -d)"

# The first pass retains no archive names and stops after the four-member bound.
# Only regular files and directories are permitted; regular files are limited
# to the exact binary and native closure. Every name is relative and safe.
if ! tar -tzf "$ARCHIVE" 2>/dev/null | awk \
  -v bin="$BIN" -v native="$EXPECTED_NATIVE" -v max="$MAX_ARCHIVE_MEMBERS" '
  BEGIN { count = 0; bin_seen = 0; native_seen = 0 }
  {
    count++
    if (count > max || $0 == "" || $0 ~ /^\// || $0 ~ /(^|\/)\.\.(\/|$)/ || $0 ~ /(^|\/)\.\/(\/|$)/ || $0 ~ /\/\// || $0 ~ /[[:cntrl:]]/) exit 1
    if ($0 == bin) {
      if (bin_seen) exit 1
      bin_seen = 1
    } else if ($0 == native) {
      if (native_seen) exit 1
      native_seen = 1
    } else if ($0 !~ /\/$/) {
      exit 1
    }
  }
  END { if (count < 2 || !bin_seen || !native_seen) exit 1 }
'; then
  echo "invalid archive: expected bounded safe members" >&2
  exit 65
fi

# Validate member types before extraction. The first character is portable
# across BSD and GNU tar listings: '-' regular, 'd' directory.
if ! tar -tvzf "$ARCHIVE" 2>/dev/null | awk \
  -v bin="$BIN" -v native="$EXPECTED_NATIVE" -v max="$MAX_ARCHIVE_MEMBERS" '
  BEGIN { count = 0; bin_seen = 0; native_seen = 0 }
  {
    count++
    type = substr($0, 1, 1)
    name = $NF
    if (count > max || (type != "-" && type != "d")) exit 1
    if (name == bin) {
      if (type != "-" || bin_seen) exit 1
      bin_seen = 1
    } else if (name == native) {
      if (type != "-" || native_seen) exit 1
      native_seen = 1
    } else if (type != "d" || name !~ /\/$/) {
      exit 1
    }
  }
  END { if (count < 2 || !bin_seen || !native_seen) exit 1 }
'; then
  echo "invalid archive: unsafe type or member" >&2
  exit 65
fi

# Probe each required stream before extraction. head bounds expansion even
# when a malicious header claims a very large regular-file payload. The
# remaining-budget subtraction avoids adding attacker-controlled byte counts.
probe_limit=$MAX_ARCHIVE_PROBE_BYTES
payload_remaining=$MAX_ARCHIVE_PAYLOAD
for member in "$BIN" "$EXPECTED_NATIVE"; do
  probe_err="$stage/probe.err"
  probe_bytes="$(tar -xOf "$ARCHIVE" "$member" 2>"$probe_err" | head -c "$probe_limit" | wc -c | tr -d '[:space:]')"
  if [ -s "$probe_err" ]; then
    echo "invalid archive: cannot read $member" >&2
    exit 65
  fi
  case "$probe_bytes" in
    ''|*[!0-9]*) echo "invalid archive: invalid expanded payload size" >&2; exit 65 ;;
  esac
  normalize_uint "$probe_bytes" || { echo "invalid archive: invalid expanded payload size" >&2; exit 65; }
  probe_bytes="$NORMALIZED_UINT"
  if uint_gt "$probe_bytes" "$MAX_ARCHIVE_MEMBER_PAYLOAD"; then
    echo "invalid archive: expanded payload exceeds $MAX_ARCHIVE_MEMBER_PAYLOAD bytes" >&2
    exit 65
  fi
  if uint_gt "$probe_bytes" "$payload_remaining"; then
    echo "invalid archive: expanded payload exceeds $MAX_ARCHIVE_PAYLOAD bytes" >&2
    exit 65
  fi
  payload_remaining=$((payload_remaining - probe_bytes))
done

if ! tar -xzf "$ARCHIVE" -C "$stage" >/dev/null 2>&1; then
  echo "invalid archive: extraction failed" >&2
  exit 65
fi
src="$stage/$BIN"
native_src="$stage/$EXPECTED_NATIVE"
[ -f "$src" ] || { echo "archive missing $BIN binary" >&2; exit 65; }
[ -f "$native_src" ] || { echo "archive missing $EXPECTED_NATIVE" >&2; exit 65; }
[ ! -L "$src" ] || { echo "invalid archive: $BIN is not regular" >&2; exit 65; }
[ ! -L "$native_src" ] || { echo "invalid archive: native library is not regular" >&2; exit 65; }

actual_payload="$(wc -c <"$src" | tr -d '[:space:]')"
native_payload="$(wc -c <"$native_src" | tr -d '[:space:]')"
case "$actual_payload" in ''|*[!0-9]*) echo "invalid archive: invalid expanded payload size" >&2; exit 65 ;; esac
case "$native_payload" in ''|*[!0-9]*) echo "invalid archive: invalid expanded payload size" >&2; exit 65 ;; esac
normalize_uint "$actual_payload" || { echo "invalid archive: invalid expanded payload size" >&2; exit 65; }
actual_payload="$NORMALIZED_UINT"
normalize_uint "$native_payload" || { echo "invalid archive: invalid expanded payload size" >&2; exit 65; }
native_payload="$NORMALIZED_UINT"
if uint_gt "$actual_payload" "$MAX_ARCHIVE_MEMBER_PAYLOAD" || uint_gt "$native_payload" "$MAX_ARCHIVE_MEMBER_PAYLOAD"; then
  echo "invalid archive: expanded payload exceeds $MAX_ARCHIVE_MEMBER_PAYLOAD bytes" >&2
  exit 65
fi
if uint_gt "$actual_payload" "$MAX_ARCHIVE_PAYLOAD"; then
  echo "invalid archive: expanded payload exceeds $MAX_ARCHIVE_PAYLOAD bytes" >&2
  exit 65
fi
native_limit=$MAX_ARCHIVE_PAYLOAD
if ! uint_gt "$actual_payload" "$native_limit"; then
  native_limit=$((native_limit - actual_payload))
  if uint_gt "$native_payload" "$native_limit"; then
    echo "invalid archive: expanded payload exceeds $MAX_ARCHIVE_PAYLOAD bytes" >&2
    exit 65
  fi
fi

# Identity is checked while still staged. A failed upgrade leaves both old
# files untouched. This also preserves the historical exit 74 contract.
chmod 755 "$src"
identity_out="$("$src" --version 2>&1)" || {
  echo "FAIL: staged binary --version failed; install unchanged" >&2
  exit 74
}
case "$identity_out" in
  *opencode-rk*)
    echo "FAIL: staged binary identifies as legacy name; install unchanged" >&2
    exit 74
    ;;
esac
case "$identity_out" in
  *oc2*) ;;
  *)
    echo "FAIL: staged binary identity mismatch (no oc2 in --version); install unchanged" >&2
    exit 74
    ;;
esac

help_out="$("$src" --help 2>&1)" || {
  echo "FAIL: staged binary --help failed; install unchanged" >&2
  exit 74
}
case "$help_out" in
  *opencode-rk*)
    echo "FAIL: staged binary --help identifies as legacy name; install unchanged" >&2
    exit 74
    ;;
esac
case "$help_out" in
  *oc2*) ;;
  *)
    echo "FAIL: staged binary identity mismatch (no oc2 in --help); install unchanged" >&2
    exit 74
    ;;
esac

target="$INSTALL_DIR/$BIN"
native_dir="$INSTALL_DIR/../lib"
native_target="$native_dir/$NATIVE_NAME"
if [ -L "$INSTALL_DIR" ] || [ -L "$native_dir" ]; then
  echo "refusing: destination directory is a symlink" >&2
  exit 74
fi
if ! mkdir -p "$INSTALL_DIR" "$native_dir"; then
  echo "FAIL: cannot create install directories" >&2
  exit 74
fi
if ! binary_tmp="$(mktemp "$INSTALL_DIR/.oc2.tmp.XXXXXX")"; then
  echo "FAIL: cannot stage installed binary" >&2
  exit 74
fi
if ! native_tmp="$(mktemp "$native_dir/.libopentui.tmp.XXXXXX")"; then
  echo "FAIL: cannot stage native library" >&2
  exit 74
fi
if ! binary_backup="$(mktemp "$INSTALL_DIR/.oc2.backup.XXXXXX")"; then
  echo "FAIL: cannot prepare binary rollback" >&2
  exit 74
fi
if ! native_backup="$(mktemp "$native_dir/.libopentui.backup.XXXXXX")"; then
  echo "FAIL: cannot prepare native rollback" >&2
  exit 74
fi
rm -f "$binary_backup" "$native_backup"
if ! cp "$src" "$binary_tmp" || ! chmod 755 "$binary_tmp"; then
  echo "FAIL: cannot stage installed binary" >&2
  exit 74
fi
if ! cp "$native_src" "$native_tmp" || ! chmod 644 "$native_tmp"; then
  echo "FAIL: cannot stage native library" >&2
  exit 74
fi

transaction_active=1
if [ -e "$target" ] || [ -L "$target" ]; then
  if ! mv "$target" "$binary_backup"; then
    rollback_install 74
  fi
  binary_backed=1
fi
if [ -e "$native_target" ] || [ -L "$native_target" ]; then
  if ! mv "$native_target" "$native_backup"; then
    rollback_install 74
  fi
  native_backed=1
fi
if ! mv "$binary_tmp" "$target"; then
  rollback_install 74
fi
binary_installed=1
if ! mv "$native_tmp" "$native_target"; then
  rollback_install 74
fi
native_installed=1
transaction_active=0
rm -f "$binary_backup" "$native_backup" 2>/dev/null || :
echo "installed $INSTALL_DIR/$BIN ($PLATFORM)" >&2
printf '%s\n' "$identity_out"
