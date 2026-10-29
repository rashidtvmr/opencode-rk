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
ALIASES="${OC2_ALIASES:-}"
INSTALL_DIR="${OC2_INSTALL_DIR:-$HOME/.local/bin}"
DO_UNINSTALL=0
MAX_ALIASES=16
MAX_ALIAS_LENGTH=64
# Two regular payloads and their three optional directory ancestors.
MAX_ARCHIVE_MEMBERS=5
# The native executable and its library total ~72 MiB on arm64 macOS. Keep
# expansion finite while admitting actual release assets on both platforms.
# Salvaged from 2f1692987ed0e27d928b290d8f2168ea37b81185.
MAX_ARCHIVE_PAYLOAD=134217728

usage() {
  echo "usage: install-oc2.sh [--version V] [--archive FILE --checksum SHA256] [--aliases a,b] [--install-dir DIR] [--uninstall]" >&2
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version) VERSION="$2"; shift 2 ;;
    --archive) ARCHIVE="$2"; shift 2 ;;
    --checksum) CHECKSUM="$2"; shift 2 ;;
    --aliases) ALIASES="$2"; shift 2 ;;
    --install-dir) INSTALL_DIR="$2"; shift 2 ;;
    OC2_RELEASE_BASE=*) OC2_RELEASE_BASE="${1#OC2_RELEASE_BASE=}"; shift ;;
    --uninstall) DO_UNINSTALL=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown flag: $1" >&2; usage; exit 64 ;;
  esac
done

# Aliases are deliberately a small, inert name list.  Do not turn this into a
# shell fragment: names are only ever passed as quoted path components.
validate_aliases() {
  alias_count=0
  old_ifs=$IFS
  IFS=,
  for alias in $ALIASES; do
    IFS=$old_ifs
    alias_count=$((alias_count + 1))
    [ "$alias_count" -le "$MAX_ALIASES" ] || { echo "too many aliases" >&2; exit 64; }
    [ -n "$alias" ] && [ "${#alias}" -le "$MAX_ALIAS_LENGTH" ] || { echo "invalid alias" >&2; exit 64; }
    case "$alias" in
      oc2|opencode|unsafe|.|..|*/*|*\\*|*" "*|*[[:cntrl:]]*)
        echo "reserved or unsafe alias: $alias" >&2; exit 64 ;;
      *[!A-Za-z0-9._-]*) echo "invalid alias: $alias" >&2; exit 64 ;;
    esac
    IFS=,
  done
  IFS=$old_ifs
}

alias_path() { printf '%s/%s' "$INSTALL_DIR" "$1"; }

validate_checksum() {
  case "$1" in
    ''|*[!0123456789abcdefABCDEF]*) echo "invalid SHA-256 checksum" >&2; exit 65 ;;
  esac
  [ "${#1}" -eq 64 ] || { echo "invalid SHA-256 checksum" >&2; exit 65; }
}

validate_aliases

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
aliases_created=""

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
  old_ifs=$IFS; IFS=,
  for alias in $aliases_created; do
    IFS=$old_ifs
    [ -z "$alias" ] || rm -f "$(alias_path "$alias")" 2>/dev/null || :
    IFS=,
  done
  IFS=$old_ifs
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
  target_inode=""
  if [ -f "$target" ] && [ ! -L "$target" ]; then
    target_inode="$(ls -di "$target" 2>/dev/null | awk '{print $1}')"
  fi
  # Metadata is untrusted: validate it before deriving any alias path.
  alias_file="$INSTALL_DIR/.oc2-aliases"
  metadata_aliases=""
  if [ -e "$alias_file" ] || [ -L "$alias_file" ]; then
    [ -f "$alias_file" ] && [ ! -L "$alias_file" ] || {
      echo "refusing: invalid alias metadata" >&2; exit 64;
    }
    metadata_bytes="$(wc -c <"$alias_file" | tr -d '[:space:]')"
    case "$metadata_bytes" in ''|*[!0-9]*) echo "refusing: invalid alias metadata" >&2; exit 64 ;; esac
    [ "$metadata_bytes" -le $((MAX_ALIASES * (MAX_ALIAS_LENGTH + 1))) ] || {
      echo "refusing: alias metadata exceeds bounds" >&2; exit 64;
    }
    metadata_aliases="$(cat "$alias_file")"
    saved_aliases="$ALIASES"
    ALIASES="$metadata_aliases"
    validate_aliases
    ALIASES="$saved_aliases"
  fi
  if [ -e "$target" ] || [ -L "$target" ]; then
    # Export marker so history tooling can re-import; never delete user data.
    echo "note: preserving user data; only removing $target" >&2
    rm -f "$target"
    echo "uninstalled $target (user data untouched)" >&2
  else
    echo "nothing to uninstall at $target" >&2
  fi
  # Remove only aliases recorded by this installer and still hard-linked to
  # the installed oc2 inode. Never unlink an unrelated replacement.
  if [ -n "$ALIASES" ]; then
    requested_aliases="$ALIASES"
  elif [ -n "$metadata_aliases" ]; then
    requested_aliases="$metadata_aliases"
  else
    requested_aliases=""
  fi
  old_ifs=$IFS; IFS=,
  for alias in $requested_aliases; do
    IFS=$old_ifs
    alias_target="$(alias_path "$alias")"
    alias_inode=""
    [ -f "$alias_target" ] && [ ! -L "$alias_target" ] && alias_inode="$(ls -di "$alias_target" 2>/dev/null | awk '{print $1}')"
    if [ -n "$target_inode" ] && [ "$alias_inode" = "$target_inode" ]; then
      rm -f "$alias_target"
    fi
    IFS=,
  done
  IFS=$old_ifs
  # Preserve the record if primary ownership is absent or ambiguous.
  if [ -n "$target_inode" ]; then
    rm -f "$alias_file" 2>/dev/null || :
  fi
  if [ -e "$native_target" ] || [ -L "$native_target" ]; then
    rm -f "$native_target"
    echo "uninstalled $native_target (user data untouched)" >&2
  fi
  exit 0
}

if [ "$DO_UNINSTALL" -eq 0 ]; then
  if [ -z "$ARCHIVE" ] && [ -n "${OC2_RELEASE_BASE:-}" ]; then
    [ -n "$VERSION" ] || { echo "download mode requires --version" >&2; exit 64; }
    case "$VERSION" in *[!A-Za-z0-9._-]*|*[.][.]*|/*|*/*) echo "invalid version" >&2; exit 64 ;; esac
  elif [ -z "$ARCHIVE" ]; then
    echo "missing --archive" >&2; usage; exit 64
  else
    [ -n "$CHECKSUM" ] || { echo "missing --checksum (fail-closed)" >&2; usage; exit 64; }
    validate_checksum "$CHECKSUM"
    [ -f "$ARCHIVE" ] || { echo "archive not found: $ARCHIVE" >&2; exit 66; }
  fi
  if [ -z "$ARCHIVE" ]; then
    case "${OC2_RELEASE_BASE}" in http://*|https://*|file://*) ;; *) echo "invalid release base" >&2; exit 64 ;; esac
  fi
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

# Collision checks happen before creating directories, temporary files, or
# replacing either payload. Existing oc2 itself remains an upgrade target;
# aliases must be absent (or already the exact installed inode on uninstall).
if [ "$DO_UNINSTALL" -eq 0 ]; then
  old_ifs=$IFS; IFS=,
  for alias in $ALIASES; do
    IFS=$old_ifs
    [ "$alias" = "$BIN" ] && { echo "alias collides with oc2" >&2; exit 64; }
    if [ -e "$(alias_path "$alias")" ] || [ -L "$(alias_path "$alias")" ]; then
      echo "alias already exists: $alias" >&2; exit 64
    fi
    IFS=,
  done
  IFS=$old_ifs
  if [ -n "$ALIASES" ] && { [ -e "$INSTALL_DIR/.oc2-aliases" ] || [ -L "$INSTALL_DIR/.oc2-aliases" ]; }; then
    echo "alias metadata already exists" >&2; exit 64
  fi
fi

if [ "$DO_UNINSTALL" -eq 0 ] && [ -z "$ARCHIVE" ]; then
  download_stage="$(mktemp -d "${TMPDIR:-/tmp}/oc2-download.XXXXXX")" || { echo "cannot create download temp" >&2; exit 74; }
  ARCHIVE="$download_stage/oc2.tar.gz"
  CHECKSUM_FILE="$download_stage/oc2.sha256"
  url="${OC2_RELEASE_BASE%/}/$VERSION/oc2-$PLATFORM.tar.gz"
  checksum_url="$url.sha256"
  command -v curl >/dev/null 2>&1 || { echo "curl required for download" >&2; exit 69; }
  curl --fail --location --proto '=https,http,file' --connect-timeout 10 --max-time 120 --max-filesize 268435456 -sS "$url" -o "$ARCHIVE" || { echo "release download failed" >&2; exit 65; }
  curl --fail --location --proto '=https,http,file' --connect-timeout 10 --max-time 30 --max-filesize 4096 -sS "$checksum_url" -o "$CHECKSUM_FILE" || { echo "checksum download failed" >&2; exit 65; }
  CHECKSUM="$(awk 'NF {print $1; exit}' "$CHECKSUM_FILE")"
  validate_checksum "$CHECKSUM"
fi

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
if ! tar -Ptzf "$ARCHIVE" 2>/dev/null | awk \
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
if ! tar -Ptvzf "$ARCHIVE" 2>/dev/null | awk \
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

# Extract only the two validated payloads. Optional directory headers can carry
# mode 000; applying those modes prevents GNU tar from creating their children.
# Let tar create the private staging ancestors instead of restoring archive modes.
if ! tar -xzf "$ARCHIVE" -C "$stage" "$BIN" "$EXPECTED_NATIVE" >/dev/null 2>&1; then
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
# Hard links deliberately share the installed binary inode and therefore also
# preserve exact bytes and executable mode. Keep the transaction armed until
# every alias and its ownership record has succeeded.
old_ifs=$IFS; IFS=,
for alias in $ALIASES; do
  IFS=$old_ifs
  alias_target="$(alias_path "$alias")"
  if ! ln "$target" "$alias_target"; then
    echo "FAIL: cannot install alias $alias" >&2
    exit 74
  fi
  if [ -n "$aliases_created" ]; then aliases_created="$aliases_created,$alias"; else aliases_created="$alias"; fi
  IFS=,
done
IFS=$old_ifs
if [ -n "$ALIASES" ]; then
  aliases_tmp="$(mktemp "$INSTALL_DIR/.oc2-aliases.tmp.XXXXXX")" || { echo "FAIL: cannot record aliases" >&2; exit 74; }
  printf '%s\n' "$ALIASES" >"$aliases_tmp" || { rm -f "$aliases_tmp"; echo "FAIL: cannot record aliases" >&2; exit 74; }
  mv "$aliases_tmp" "$INSTALL_DIR/.oc2-aliases" || { rm -f "$aliases_tmp"; echo "FAIL: cannot record aliases" >&2; exit 74; }
fi
transaction_active=0
echo "installed $INSTALL_DIR/$BIN ($PLATFORM)" >&2
printf '%s\n' "$identity_out"
