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
# Two regular payloads and their three optional directory ancestors.
MAX_ARCHIVE_MEMBERS=5
# The native executable and its library total ~72 MiB on arm64 macOS. Keep
# expansion finite while admitting actual release assets on both platforms.
# Salvaged from 2f1692987ed0e27d928b290d8f2168ea37b81185.
MAX_ARCHIVE_PAYLOAD=134217728

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
binary_backup_dir=""
native_backup_dir=""
binary_backup=""
native_backup=""
binary_had_old=0
native_had_old=0
binary_swap_started=0
native_swap_started=0
binary_restore_failed=0
native_restore_failed=0

restore_previous() {
  # A failing rename can also affect rollback. Keep a copy fallback and retain
  # the backup if both operations fail, rather than claiming an unchanged install.
  if mv "$1" "$2" 2>/dev/null || cp -pP "$1" "$2" 2>/dev/null; then
    return 0
  fi
  echo "FAIL: cannot restore $2; previous file preserved at $1" >&2
  return 1
}

rollback_install() {
  transaction_active=0
  trap '' HUP INT TERM
  if [ "$binary_swap_started" -eq 1 ]; then
    if [ "$binary_had_old" -eq 1 ]; then
      restore_previous "$binary_backup" "$target" || binary_restore_failed=1
    else
      rm -f "$target" 2>/dev/null || :
    fi
  fi
  if [ "$native_swap_started" -eq 1 ]; then
    if [ "$native_had_old" -eq 1 ]; then
      restore_previous "$native_backup" "$native_target" || native_restore_failed=1
    else
      rm -f "$native_target" 2>/dev/null || :
    fi
  fi
  if [ "$binary_restore_failed" -eq 0 ] && [ "$native_restore_failed" -eq 0 ]; then
    echo "FAIL: install transaction rolled back" >&2
  fi
}

cleanup() {
  if [ "$transaction_active" -eq 1 ]; then
    rollback_install
  fi
  [ -z "$stage" ] || rm -rf "$stage" 2>/dev/null || :
  rm -f "$binary_tmp" "$native_tmp" 2>/dev/null || :
  if [ "$binary_restore_failed" -eq 0 ] && [ -n "$binary_backup_dir" ]; then
    rm -rf "$binary_backup_dir" 2>/dev/null || :
  fi
  if [ "$native_restore_failed" -eq 0 ] && [ -n "$native_backup_dir" ]; then
    rm -rf "$native_backup_dir" 2>/dev/null || :
  fi
}

trap cleanup 0
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

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
if [ -e "$INSTALL_DIR/opencode" ]; then
  echo "refusing: $INSTALL_DIR/opencode exists; oc2 installs side-by-side only" >&2
  exit 73
fi

stage="$(mktemp -d)"

# Bounded safe-member gate: only regular files and directories, exact binary
# plus exact native closure and only its directory ancestors, five-member bound.
# Salvaged from cc3ebaa/2f16929; no manifest/receipt producer here.
if ! tar -tzf "$ARCHIVE" 2>/dev/null | awk \
  -v bin="$BIN" -v native="$EXPECTED_NATIVE" -v max="$MAX_ARCHIVE_MEMBERS" '
  BEGIN { count = 0; bin_seen = 0; native_seen = 0 }
  {
    count++
    if (count > max || $0 == "" || $0 ~ /^\// || $0 ~ /(^|\/)\.\.(\/|$)/ || $0 ~ /(^|\/)\.\/(\/|$)/ || $0 ~ /\/\// || $0 ~ /[[:cntrl:]]/ || seen[$0]++) exit 1
    if ($0 == bin) {
      if (bin_seen) exit 1
      bin_seen = 1
    } else if ($0 == native) {
      if (native_seen) exit 1
      native_seen = 1
    } else if ($0 !~ /\/$/ || index(native, $0) != 1) {
      exit 1
    }
  }
  END { if (count < 2 || !bin_seen || !native_seen) exit 1 }
'; then
  echo "invalid archive: expected bounded safe members" >&2
  exit 65
fi

# Validate member types before extraction. First character is portable across
# BSD and GNU tar listings: '-' regular, 'd' directory.
if ! tar -tvzf "$ARCHIVE" 2>/dev/null | awk \
  -v bin="$BIN" -v native="$EXPECTED_NATIVE" -v max="$MAX_ARCHIVE_MEMBERS" '
  BEGIN { count = 0; bin_seen = 0; native_seen = 0 }
  {
    count++
    type = substr($0, 1, 1)
    name = $NF
    if (count > max || (type != "-" && type != "d") || seen[name]++) exit 1
    if (name == bin) {
      if (type != "-" || bin_seen) exit 1
      bin_seen = 1
    } else if (name == native) {
      if (type != "-" || native_seen) exit 1
      native_seen = 1
    } else if (type != "d" || name !~ /\/$/ || index(native, name) != 1) {
      exit 1
    }
  }
  END { if (count < 2 || !bin_seen || !native_seen) exit 1 }
'; then
  echo "invalid archive: unsafe type or member" >&2
  exit 65
fi

# Probe each required stream before extraction. head bounds expansion even
# when a malicious header claims a very large regular-file payload.
payload=0
for member in "$BIN" "$EXPECTED_NATIVE"; do
  probe_err="$stage/probe.err"
  probe_bytes="$(tar -xOf "$ARCHIVE" "$member" 2>"$probe_err" | head -c $((MAX_ARCHIVE_PAYLOAD + 1)) | wc -c | tr -d '[:space:]')"
  if [ -s "$probe_err" ]; then
    echo "invalid archive: cannot read $member" >&2
    exit 65
  fi
  case "$probe_bytes" in
    ''|*[!0-9]*) echo "invalid archive: invalid expanded payload size" >&2; exit 65 ;;
  esac
  if [ "$probe_bytes" -gt "$MAX_ARCHIVE_PAYLOAD" ]; then
    echo "invalid archive: expanded payload exceeds $MAX_ARCHIVE_PAYLOAD bytes" >&2
    exit 65
  fi
  payload=$((payload + probe_bytes))
  if [ "$payload" -gt "$MAX_ARCHIVE_PAYLOAD" ]; then
    echo "invalid archive: expanded payload exceeds $MAX_ARCHIVE_PAYLOAD bytes" >&2
    exit 65
  fi
done

if ! tar -xzf "$ARCHIVE" -C "$stage" >/dev/null 2>&1; then
  echo "invalid archive: extraction failed" >&2
  exit 65
fi
# Archive directory modes must not prevent traversing the private staging tree.
chmod u+rwx "$stage/native"
chmod u+rwx "$stage/native/lib"
chmod u+rwx "$stage/native/lib/$PLATFORM"
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
expanded_payload=$((actual_payload + native_payload))
[ "$expanded_payload" -le "$MAX_ARCHIVE_PAYLOAD" ] || {
  echo "invalid archive: expanded payload exceeds $MAX_ARCHIVE_PAYLOAD bytes" >&2
  exit 65
}

# Identity is checked while still staged. Mirror the installed bin/../lib
# layout first: a dynamic oc2 cannot start from the archive root because its
# relative loader path would otherwise have no matching native library.
# Salvaged from 2f16929. A failed install leaves both old files untouched.
# Preserves the historical exit 74 contract.
if ! mkdir -p "$stage/bin" "$stage/lib" ||
   ! mv "$src" "$stage/bin/$BIN" ||
   ! mv "$native_src" "$stage/lib/$NATIVE_NAME"; then
  echo "FAIL: cannot stage native release layout; install unchanged" >&2
  exit 74
fi
src="$stage/bin/$BIN"
native_src="$stage/lib/$NATIVE_NAME"
chmod 755 "$src"
# Packaged-output identity check (APP-010): the installed binary must
# identify as oc2 and never as the legacy dev name. Mirrors
# packaged_output_names_oc2 in crates/cli/src/install_commands.rs.
# Staged check: a failure leaves the existing install untouched (exit 74).
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
# Prepare both replacements and backups beside their targets. Backing up by
# copy keeps the previous install usable until the first replacement begins.
if ! binary_tmp="$(mktemp "$INSTALL_DIR/.oc2.tmp.XXXXXX")"; then
  echo "FAIL: cannot stage installed binary" >&2
  exit 74
fi
if ! native_tmp="$(mktemp "$native_dir/.libopentui.tmp.XXXXXX")"; then
  echo "FAIL: cannot stage native library" >&2
  exit 74
fi
if ! binary_backup_dir="$(mktemp -d "$INSTALL_DIR/.oc2.backup.XXXXXX")" ||
   ! native_backup_dir="$(mktemp -d "$native_dir/.libopentui.backup.XXXXXX")"; then
  echo "FAIL: cannot prepare install rollback" >&2
  exit 74
fi
binary_backup="$binary_backup_dir/$BIN"
native_backup="$native_backup_dir/$NATIVE_NAME"
if ! cp "$src" "$binary_tmp" || ! chmod 755 "$binary_tmp"; then
  echo "FAIL: cannot stage installed binary" >&2
  exit 74
fi
if ! cp "$native_src" "$native_tmp" || ! chmod 644 "$native_tmp"; then
  echo "FAIL: cannot stage native library" >&2
  exit 74
fi
if [ -e "$target" ] || [ -L "$target" ]; then
  if ! cp -pP "$target" "$binary_backup"; then
    echo "FAIL: cannot back up installed binary; install unchanged" >&2
    exit 74
  fi
  binary_had_old=1
  echo "upgrade: preserving existing install (history lives in user data dir, untouched)" >&2
fi
if [ -e "$native_target" ] || [ -L "$native_target" ]; then
  if ! cp -pP "$native_target" "$native_backup"; then
    echo "FAIL: cannot back up installed native library; install unchanged" >&2
    exit 74
  fi
  native_had_old=1
fi
# Arm each replacement before its syscall so a signal immediately after a
# successful rename still rolls back. All three traps exit through cleanup.
transaction_active=1
native_swap_started=1
mv "$native_tmp" "$native_target" || {
  echo "FAIL: cannot install native library" >&2
  exit 74
}
binary_swap_started=1
mv "$binary_tmp" "$target" || {
  echo "FAIL: cannot install binary" >&2
  exit 74
}
transaction_active=0
echo "installed $INSTALL_DIR/$BIN ($PLATFORM)" >&2
printf '%s\n' "$identity_out"
