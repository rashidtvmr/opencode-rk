#!/usr/bin/env sh
# install-oc2.sh: install one externally supplied oc2 release bundle.
# This is an unsigned integrity consumer, not a producer, signer, or attestor.
set -eu

LC_ALL=C
export LC_ALL
PATH=/usr/bin:/bin:/usr/sbin:/sbin
export PATH
umask 077

BIN=oc2
VERSION=""
ARCHIVE=""
CHECKSUM=""
MANIFEST=""
INSTALL_DIR=""
DO_UNINSTALL=0

MAX_ARCHIVE_BYTES=134217728
MAX_MEMBER_BYTES=134217728
MAX_TOTAL_MEMBER_BYTES=134217728
MAX_USTAR_BYTES=134221824
MAX_SBOM_BYTES=65536
MAX_MANIFEST_BYTES=65536
MAX_RUNTIME_OUTPUT=4096
RUNTIME_TIMEOUT_SECONDS=5

# Temporary output files use four 512-byte file-size blocks each. The explicit
# post-wait sum enforces the 4096-byte combined stdout/stderr ceiling.
RUNTIME_FILE_BLOCKS=4

stage=""
transaction_active=0
target=""
native_dir=""
native_target=""
binary_tmp=""
native_tmp=""
binary_backup=""
native_backup=""
binary_backed=0
native_backed=0
binary_installed=0
native_installed=0
runtime_pid=""
runtime_watchdog=""
runtime_active=0

FILE_SIZE=""
NORMALIZED_UINT=""
PROBE_BYTES=""
PAYLOAD_REMAINING=""
RUNTIME_LINE=""
STDOUT_SIZE=""
STDERR_SIZE=""

usage() {
  echo "usage: install-oc2.sh --archive FILE --checksum SHA256 --manifest FILE [--install-dir DIR] [--uninstall]" >&2
}

integrity_fail() {
  echo "FAIL: $1" >&2
  exit 65
}

identity_fail() {
  echo "FAIL: $1" >&2
  exit 74
}

require_value() {
  if [ "$1" -lt 2 ]; then
    echo "missing value for $2" >&2
    usage
    exit 64
  fi
}

is_lower_hex_64() {
  [ "${#1}" -eq 64 ] || return 1
  case "$1" in
    *[!0-9a-f]*) return 1 ;;
    *) return 0 ;;
  esac
}

is_lower_hex_40() {
  [ "${#1}" -eq 40 ] || return 1
  case "$1" in
    *[!0-9a-f]*) return 1 ;;
    *) return 0 ;;
  esac
}

normalize_uint() {
  case "$1" in
    ''|*[!0-9]*) return 1 ;;
  esac
  NORMALIZED_UINT="$1"
  while [ "$NORMALIZED_UINT" != "0" ] && [ "${NORMALIZED_UINT#0}" != "$NORMALIZED_UINT" ]; do
    NORMALIZED_UINT="${NORMALIZED_UINT#0}"
  done
}

# Compare untrusted decimal text without arithmetic on attacker-sized operands.
uint_gt() {
  _left="$1"
  _right="$2"
  normalize_uint "$_left" || return 2
  _left="$NORMALIZED_UINT"
  normalize_uint "$_right" || return 2
  _right="$NORMALIZED_UINT"
  _left_len=${#_left}
  _right_len=${#_right}
  if [ "$_left_len" -gt "$_right_len" ]; then
    return 0
  fi
  if [ "$_left_len" -lt "$_right_len" ]; then
    return 1
  fi
  LC_ALL=C awk -v left="x$_left" -v right="x$_right" 'BEGIN { exit !(left > right) }'
}

measure_file() {
  _measured=$(wc -c <"$1" 2>/dev/null) || return 1
  _measured=$(printf '%s' "$_measured" | tr -d '[:space:]')
  normalize_uint "$_measured" || return 1
  FILE_SIZE="$NORMALIZED_UINT"
}

sha256_file() {
  _digest_output=""
  if command -v sha256sum >/dev/null 2>&1; then
    _digest_output=$(sha256sum "$1" 2>/dev/null) || return 1
  elif command -v shasum >/dev/null 2>&1; then
    _digest_output=$(shasum -a 256 "$1" 2>/dev/null) || return 1
  else
    return 1
  fi
  _digest=${_digest_output%% *}
  is_lower_hex_64 "$_digest" || return 1
  printf '%s' "$_digest"
}

require_tool() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "required installer tool unavailable: $1" >&2
    exit 64
  }
}

# This is deliberately a closed parser for the one exact canonical schema, not
# a general JSON parser and not an eval. It accepts only seven fixed top-level
# tokens (the members object is one token), ASCII bytes, exact key order, exact
# types, the host's exact three names, and one final byte. The closed grammar
# has a fixed token count, so oversized/nested/unknown JSON
# cannot be admitted.
parse_manifest() {
  od -A n -v -t x1 "$1" | awk \
    -v expected_platform="$PLATFORM" \
    -v expected_native="$EXPECTED_NATIVE" \
    -v max_bytes="$MAX_MANIFEST_BYTES" '
    BEGIN { digits = "0123456789abcdef"; bad = 0 }
    function reject() { bad = 1; exit 1 }
    function byte_at(position, pair) {
      pair = substr(hex, position, 2)
      if (length(pair) != 2 || pair ~ /[^0-9a-f]/) reject()
      return (index(digits, substr(pair, 1, 1)) - 1) * 16 + \
             (index(digits, substr(pair, 2, 1)) - 1)
    }
    function take(count) {
      value = substr(source, position, count)
      position += count
      return value
    }
    function expect(expected, actual) {
      actual = take(length(expected))
      if (actual != expected) reject()
    }
    function take_hash_64() {
      value = take(64)
      if (value !~ /^[0-9a-f]+$/) reject()
      return value
    }
    {
      for (field = 1; field <= NF; field++) hex = hex $field
    }
    END {
      if (bad) exit 1
      if (length(hex) % 2 != 0) reject()
      if (length(hex) / 2 > max_bytes) reject()

      source = ""
      for (offset = 1; offset <= length(hex); offset += 2) {
        value = byte_at(offset)
        # Canonical v1 contains ASCII only. This also rejects UTF-8, NUL,
        # controls, BOM, whitespace, DEL, and every noncanonical escape.
        if (value < 32 || value > 126) reject()
        source = source sprintf("%c", value)
      }

      position = 1
      expect("{\"archive_sha256\":\"")
      archive_sha256 = take_hash_64()
      expect("\",\"members\":{")
      expect("\"" expected_native "\":\"")
      native_sha256 = take_hash_64()
      expect("\",\"oc2\":\"")
      binary_sha256 = take_hash_64()
      expect("\",\"sbom.json\":\"")
      sbom_sha256 = take_hash_64()
      expect("\"},\"platform\":\"")
      manifest_platform = take(length(expected_platform))
      if (manifest_platform != expected_platform) reject()
      expect("\",\"revision\":\"")
      revision = take(40)
      if (revision !~ /^[0-9a-f]+$/) reject()
      expect("\",\"sbom\":{\"path\":\"sbom.json\",\"sha256\":\"")
      sbom_field_sha256 = take_hash_64()
      expect("\"},\"schema\":\"oc2-release-receipt/v1\"}")
      if (position != length(source) + 1) reject()
      if (sbom_field_sha256 != sbom_sha256) reject()

      printf "archive_sha256=%s\nbinary_sha256=%s\nnative_sha256=%s\nsbom_sha256=%s\nplatform=%s\nrevision=%s\n", \
        archive_sha256, binary_sha256, native_sha256, sbom_sha256, manifest_platform, revision
    }'
}

decompress_bounded_archive() {
  tar_bytes="$stage/archive.tar"

  # gzip writes into a bounded file under a per-process file-size limit. This
  # avoids the POSIX-pipeline status gap that otherwise hides SIGPIPE after
  # head truncates an expansion bomb.
  _ustar_file_blocks=$(((MAX_USTAR_BYTES + 512) / 512))
  if ! (
    ulimit -f "$_ustar_file_blocks" 2>/dev/null || exit 125
    gzip -cd "$archive_copy" >"$tar_bytes"
  ) 2>/dev/null; then
    integrity_fail "cannot decompress bounded USTAR stream"
  fi
  measure_file "$tar_bytes" || integrity_fail "cannot measure USTAR stream"
  if uint_gt "$FILE_SIZE" "$MAX_USTAR_BYTES"; then
    integrity_fail "USTAR stream exceeds structural byte limit"
  fi
}

validate_gzip_header() {
  if ! dd if="$archive_copy" bs=1 count=10 2>/dev/null |
    od -A n -v -t x1 |
    tr -d '[:space:]' |
    awk '
    {
      for (field = 1; field <= NF; field++) header = header $field
    }
    END {
      if (length(header) != 20 || header != "1f8b0800000000000203") exit 1
    }'; then
    integrity_fail "gzip header is not canonical"
  fi
}

read_ustar_size() {
  _size_offset=$1
  dd if="$tar_bytes" bs=512 skip="$((_size_offset / 512))" count=1 2>/dev/null |
    od -A n -v -t x1 |
    tr -d '[:space:]' |
    awk '
    BEGIN { digits = "0123456789abcdef" }
    function byte_at(position, pair) {
      pair = substr(header, position, 2)
      if (length(pair) != 2 || pair ~ /[^0-9a-f]/) exit 1
      return (index(digits, substr(pair, 1, 1)) - 1) * 16 + \
             (index(digits, substr(pair, 2, 1)) - 1)
    }
    {
      for (field = 1; field <= NF; field++) header = header $field
    }
    END {
      if (length(header) != 1024) exit 1
      value = 0
      for (position = 249; position <= 269; position += 2) {
        digit = byte_at(position)
        if (digit < 48 || digit > 55) exit 1
        value = value * 8 + digit - 48
      }
      if (byte_at(271) != 0) exit 1
      print value
    }'
}

extract_member() {
  _extract_offset=$1
  _extract_size=$2
  _extract_path=$3
  _extract_blocks=$(((_extract_size + 511) / 512))
  if ! dd if="$tar_bytes" bs=512 skip="$((_extract_offset / 512))" count="$_extract_blocks" 2>/dev/null |
    head -c "$_extract_size" >"$_extract_path"; then
    return 1
  fi
  measure_file "$_extract_path" || return 1
  [ "$FILE_SIZE" = "$_extract_size" ]
}

validate_ustar_header() {
  _header_offset=$1
  _header_name=$2
  _header_mode=$3
  _header_size=$4
  _header_name_hex=$(printf '%s' "$_header_name" | od -A n -v -t x1 | tr -d '[:space:]')

  dd if="$tar_bytes" bs=512 skip="$((_header_offset / 512))" count=1 2>/dev/null |
    od -A n -v -t x1 |
    tr -d '[:space:]' |
    awk -v name_hex="$_header_name_hex" -v mode_value="$_header_mode" -v size_value="$_header_size" '
    BEGIN { digits = "0123456789abcdef" }
    function byte_at(position, pair) {
      pair = substr(header, position, 2)
      if (length(pair) != 2 || pair ~ /[^0-9a-f]/) return -1
      return (index(digits, substr(pair, 1, 1)) - 1) * 16 + \
             (index(digits, substr(pair, 2, 1)) - 1)
    }
    function all_zero(start, end, position) {
      for (position = start; position <= end; position += 2)
        if (byte_at(position) != 0) return 0
      return 1
    }
    function octal_field_hex(text, output, position, digit) {
      output = ""
      for (position = 1; position <= length(text); position++) {
        digit = index("01234567", substr(text, position, 1))
        if (digit == 0) return "!"
        output = output sprintf("%02x", 47 + digit)
      }
      return output "00"
    }
    {
      for (field = 1; field <= NF; field++) header = header $field
    }
    END {
      if (length(header) != 1024) exit 1
      if (substr(header, 1, length(name_hex)) != name_hex) exit 1
      if (!all_zero(length(name_hex) + 1, 200)) exit 1

      if (substr(header, 201, 16) != octal_field_hex(sprintf("%07o", mode_value))) exit 1
      if (substr(header, 217, 16) != octal_field_hex(sprintf("%07o", 0))) exit 1
      if (substr(header, 233, 16) != octal_field_hex(sprintf("%07o", 0))) exit 1
      if (substr(header, 249, 24) != octal_field_hex(sprintf("%011o", size_value))) exit 1
      if (substr(header, 273, 24) != octal_field_hex(sprintf("%011o", 0))) exit 1
      if (substr(header, 313, 2) != "30") exit 1
      if (!all_zero(315, 514)) exit 1
      if (substr(header, 515, 12) != "757374617200") exit 1
      if (substr(header, 527, 4) != "3030") exit 1
      if (!all_zero(531, 1024)) exit 1

      stored_checksum = 0
      for (position = 297; position <= 307; position += 2) {
        value = byte_at(position)
        if (value < 48 || value > 55) exit 1
        stored_checksum = stored_checksum * 8 + value - 48
      }
      if (byte_at(309) != 0 || byte_at(311) != 32) exit 1
      computed_checksum = 0
      for (position = 1; position <= 1024; position += 2) {
        value = byte_at(position)
        if (position >= 297 && position <= 312) value = 32
        computed_checksum += value
      }
      if (stored_checksum != computed_checksum) exit 1
    }'
}

validate_ustar_headers() {
  header_offset=0
  validate_ustar_header "$header_offset" "$EXPECTED_NATIVE" 420 "$probed_native_bytes" ||
    integrity_fail "invalid native USTAR header"
  header_offset=$((header_offset + 512 + (probed_native_bytes + 511) / 512 * 512))
  validate_ustar_header "$header_offset" "$BIN" 493 "$probed_binary_bytes" ||
    integrity_fail "invalid binary USTAR header"
  header_offset=$((header_offset + 512 + (probed_binary_bytes + 511) / 512 * 512))
  validate_ustar_header "$header_offset" sbom.json 420 "$probed_sbom_bytes" ||
    integrity_fail "invalid SBOM USTAR header"
  header_offset=$((header_offset + 512 + (probed_sbom_bytes + 511) / 512 * 512))

  if ! dd if="$tar_bytes" bs=512 skip="$((header_offset / 512))" count=2 2>/dev/null |
    od -A n -v -t x1 |
    tr -d '[:space:]' |
    awk '
    {
      for (field = 1; field <= NF; field++) header = header $field
    }
    END {
      if (length(header) != 2048 || header ~ /[^0]/) exit 1
    }'; then
    integrity_fail "USTAR stream lacks exact zero EOF blocks"
  fi
  expected_ustar_bytes=$((header_offset + 1024))
  measure_file "$tar_bytes" || integrity_fail "cannot measure validated USTAR stream"
  [ "$FILE_SIZE" -eq "$expected_ustar_bytes" ] || integrity_fail "USTAR stream has trailing data"
}

verify_runtime_receipt() {
  _runtime_binary=$1
  _stdout="$stage/runtime.stdout"
  _stderr="$stage/runtime.stderr"
  _timeout_marker="$stage/runtime.timeout"
  rm -f "$_stdout" "$_stderr" "$_timeout_marker"
  : >"$_stdout"
  : >"$_stderr"

  # Receipt identity is compile-time only. Remove known test/build fallbacks so
  # an environment-provided revision cannot satisfy either observation.
  unset OC2_E2E_REVISION OC2_BUILD_REVISION GIT_COMMIT

  # Prefer a new session where util-linux setsid is available. On systems
  # without it (notably macOS), non-interactive job control gives the child a
  # separate process group. The watchdog kills and the parent waits for the
  # owned receipt process; group kill closes descendants where either facility
  # is available.
  set -m 2>/dev/null || :
  if command -v setsid >/dev/null 2>&1; then
    (
      ulimit -f "$RUNTIME_FILE_BLOCKS" 2>/dev/null || exit 125
      exec setsid env -i PATH="$PATH" LC_ALL=C "$_runtime_binary" --version
    ) >"$_stdout" 2>"$_stderr" &
  else
    (
      ulimit -f "$RUNTIME_FILE_BLOCKS" 2>/dev/null || exit 125
      exec env -i PATH="$PATH" LC_ALL=C "$_runtime_binary" --version
    ) >"$_stdout" 2>"$_stderr" &
  fi
  runtime_pid=$!
  set +m 2>/dev/null || :
  runtime_active=1

  (
    sleep "$RUNTIME_TIMEOUT_SECONDS"
    if kill -0 "$runtime_pid" 2>/dev/null; then
      : >"$_timeout_marker"
      kill -TERM "-$runtime_pid" 2>/dev/null || kill -TERM "$runtime_pid" 2>/dev/null || :
      kill -KILL "-$runtime_pid" 2>/dev/null || kill -KILL "$runtime_pid" 2>/dev/null || :
    fi
  ) >/dev/null 2>&1 &
  runtime_watchdog=$!

  if wait "$runtime_pid"; then
    _runtime_status=0
  else
    _runtime_status=$?
  fi
  kill -TERM "-$runtime_pid" 2>/dev/null || :
  kill -KILL "-$runtime_pid" 2>/dev/null || :
  runtime_active=0
  kill "$runtime_watchdog" 2>/dev/null || :
  wait "$runtime_watchdog" 2>/dev/null || :
  runtime_pid=""
  runtime_watchdog=""

  [ ! -e "$_timeout_marker" ] || return 1
  [ "$_runtime_status" -eq 0 ] || return 1
  measure_file "$_stdout" || return 1
  STDOUT_SIZE="$FILE_SIZE"
  measure_file "$_stderr" || return 1
  STDERR_SIZE="$FILE_SIZE"
  [ "$STDERR_SIZE" -eq 0 ] || return 1
  _runtime_total=$((STDOUT_SIZE + STDERR_SIZE))
  if uint_gt "$_runtime_total" "$MAX_RUNTIME_OUTPUT"; then
    return 1
  fi

  RUNTIME_LINE=$(cat "$_stdout" 2>/dev/null) || return 1
  case "$RUNTIME_LINE" in
    "oc2 "*) _runtime_rest=${RUNTIME_LINE#oc2 } ;;
    *) return 1 ;;
  esac
  _runtime_suffix=" revision=$MANIFEST_REVISION"
  case "$_runtime_rest" in
    *"$_runtime_suffix") _package_version=${_runtime_rest%"$_runtime_suffix"} ;;
    *) return 1 ;;
  esac
  [ -n "$_package_version" ] || return 1
  case "$_package_version" in
    *[!A-Za-z0-9.+-]*) return 1 ;;
  esac

  # Command substitution removes trailing newlines. Equality to one LF rejects
  # missing LF, CR, extra LF, embedded LF, trailing text, and NUL truncation.
  [ "$STDOUT_SIZE" -eq $(( ${#RUNTIME_LINE} + 1 )) ] || return 1
}

stop_runtime() {
  if [ -n "$runtime_watchdog" ]; then
    kill "$runtime_watchdog" 2>/dev/null || :
    wait "$runtime_watchdog" 2>/dev/null || :
    runtime_watchdog=""
  fi
  if [ "$runtime_active" -eq 1 ] && [ -n "$runtime_pid" ]; then
    kill -TERM "-$runtime_pid" 2>/dev/null || kill -TERM "$runtime_pid" 2>/dev/null || :
    sleep 1
    kill -KILL "-$runtime_pid" 2>/dev/null || kill -KILL "$runtime_pid" 2>/dev/null || :
    wait "$runtime_pid" 2>/dev/null || :
  fi
  runtime_active=0
  runtime_pid=""
}

cleanup() {
  stop_runtime
  if [ -n "$stage" ]; then
    rm -rf "$stage" 2>/dev/null || :
  fi
  rm -f "$binary_tmp" "$native_tmp" 2>/dev/null || :
  if [ "$transaction_active" -eq 0 ] || [ "$binary_backed" -eq 0 ]; then
    rm -f "$binary_backup" 2>/dev/null || :
  fi
  if [ "$transaction_active" -eq 0 ] || [ "$native_backed" -eq 0 ]; then
    rm -f "$native_backup" 2>/dev/null || :
  fi
}

rollback_install() {
  rollback_code=$1
  transaction_active=0
  stop_runtime

  if [ "$native_installed" -eq 1 ]; then
    rm -f "$native_target" 2>/dev/null || :
  fi
  if [ "$binary_installed" -eq 1 ]; then
    rm -f "$target" 2>/dev/null || :
  fi
  if [ "$native_backed" -eq 1 ]; then
    if ! mv -f "$native_backup" "$native_target" 2>/dev/null; then
      echo "FAIL: could not restore previous native library" >&2
    fi
  fi
  if [ "$binary_backed" -eq 1 ]; then
    if ! mv -f "$binary_backup" "$target" 2>/dev/null; then
      echo "FAIL: could not restore previous binary" >&2
    fi
  fi
  rm -f "$binary_tmp" "$native_tmp" 2>/dev/null || :
  echo "FAIL: install transaction rolled back" >&2
  exit "$rollback_code"
}

on_signal() {
  signal_code=$1
  if [ "$transaction_active" -eq 1 ]; then
    rollback_install "$signal_code"
  fi
  stop_runtime
  exit "$signal_code"
}

trap cleanup 0
trap 'on_signal 130' 2
trap 'on_signal 143' 15

while [ $# -gt 0 ]; do
  case "$1" in
    --version)
      require_value "$#" "--version"
      VERSION="$2"
      shift 2
      ;;
    --archive)
      require_value "$#" "--archive"
      ARCHIVE="$2"
      shift 2
      ;;
    --checksum)
      require_value "$#" "--checksum"
      CHECKSUM="$2"
      shift 2
      ;;
    --manifest)
      require_value "$#" "--manifest"
      MANIFEST="$2"
      shift 2
      ;;
    --install-dir)
      require_value "$#" "--install-dir"
      INSTALL_DIR="$2"
      shift 2
      ;;
    --uninstall)
      DO_UNINSTALL=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown flag: $1" >&2
      usage
      exit 64
      ;;
  esac
done

if [ -z "$INSTALL_DIR" ]; then
  if [ -n "${HOME:-}" ]; then
    INSTALL_DIR="$HOME/.local/bin"
  else
    echo "missing --install-dir or HOME" >&2
    usage
    exit 64
  fi
fi

[ -z "$ARCHIVE" ] || { case "$ARCHIVE" in /*) ;; *) ARCHIVE="./$ARCHIVE" ;; esac; }
[ -z "$MANIFEST" ] || { case "$MANIFEST" in /*) ;; *) MANIFEST="./$MANIFEST" ;; esac; }
case "$INSTALL_DIR" in /*) ;; *) INSTALL_DIR="./$INSTALL_DIR" ;; esac

for required_tool in awk cat chmod cp dd env grep gzip head iconv mkdir mktemp mv od rm sleep tr uname wc; do
  require_tool "$required_tool"
done
if ! command -v sha256sum >/dev/null 2>&1 && ! command -v shasum >/dev/null 2>&1; then
  echo "required installer tool unavailable: sha256sum or shasum" >&2
  exit 64
fi

detect_platform() {
  os=$(uname -s)
  arch=$(uname -m)
  case "$os" in
    Linux) platform_os=linux ;;
    Darwin) platform_os=macos ;;
    *) echo "unsupported OS: $os" >&2; exit 64 ;;
  esac
  case "$arch" in
    x86_64|amd64) platform_arch=x64 ;;
    arm64|aarch64) platform_arch=arm64 ;;
    *) echo "unsupported arch: $arch" >&2; exit 64 ;;
  esac
  PLATFORM="$platform_os-$platform_arch"
  case "$PLATFORM" in
    linux-x64|linux-arm64|macos-x64|macos-arm64) ;;
    *) echo "unsupported platform: $PLATFORM" >&2; exit 64 ;;
  esac
}

detect_platform
case "$PLATFORM" in
  linux-*) NATIVE_NAME=libopentui.so ;;
  macos-*) NATIVE_NAME=libopentui.dylib ;;
  *) echo "unsupported platform: $PLATFORM" >&2; exit 64 ;;
esac
EXPECTED_NATIVE="native/lib/$PLATFORM/$NATIVE_NAME"

target="$INSTALL_DIR/$BIN"
native_dir="$INSTALL_DIR/../lib"
native_target="$native_dir/$NATIVE_NAME"

if [ "$DO_UNINSTALL" -eq 1 ]; then
  if [ -L "$INSTALL_DIR" ] || [ -L "$native_dir" ]; then
    identity_fail "destination directory is a symlink"
  fi
  if [ -e "$target" ] || [ -L "$target" ]; then
    echo "note: preserving user data; only removing $target" >&2
    rm -f "$target" || identity_fail "cannot remove $target"
    echo "uninstalled $target (user data untouched)" >&2
  else
    echo "nothing to uninstall at $target" >&2
  fi
  if [ -e "$native_target" ] || [ -L "$native_target" ]; then
    rm -f "$native_target" || identity_fail "cannot remove $native_target"
    echo "uninstalled $native_target (user data untouched)" >&2
  fi
  exit 0
fi

[ -n "$ARCHIVE" ] || { echo "missing --archive" >&2; usage; exit 64; }
[ -n "$CHECKSUM" ] || { echo "missing --checksum (fail-closed)" >&2; usage; exit 64; }
[ -n "$MANIFEST" ] || { echo "missing --manifest (fail-closed)" >&2; usage; exit 64; }
is_lower_hex_64 "$CHECKSUM" || { echo "malformed --checksum" >&2; exit 64; }

if [ ! -e "$ARCHIVE" ] && [ ! -L "$ARCHIVE" ]; then
  echo "archive not found" >&2
  exit 66
fi
if [ -L "$ARCHIVE" ] || [ ! -f "$ARCHIVE" ]; then
  integrity_fail "archive must be a regular non-symlink file"
fi
if [ ! -e "$MANIFEST" ] && [ ! -L "$MANIFEST" ]; then
  echo "manifest not found" >&2
  exit 66
fi
if [ -L "$MANIFEST" ] || [ ! -f "$MANIFEST" ]; then
  integrity_fail "manifest must be a regular non-symlink file"
fi
if [ -L "$INSTALL_DIR" ] || [ -L "$native_dir" ]; then
  identity_fail "destination directory is a symlink"
fi
if [ -e "$INSTALL_DIR/opencode" ] || [ -L "$INSTALL_DIR/opencode" ]; then
  echo "refusing: $INSTALL_DIR/opencode exists; oc2 installs side-by-side only" >&2
  exit 73
fi

measure_file "$ARCHIVE" || integrity_fail "cannot measure archive"
if uint_gt "$FILE_SIZE" "$MAX_ARCHIVE_BYTES"; then
  integrity_fail "compressed archive exceeds $MAX_ARCHIVE_BYTES bytes"
fi
measure_file "$MANIFEST" || integrity_fail "cannot measure manifest"
if uint_gt "$FILE_SIZE" "$MAX_MANIFEST_BYTES"; then
  integrity_fail "manifest exceeds $MAX_MANIFEST_BYTES bytes"
fi

stage=$(mktemp -d 2>/dev/null) || identity_fail "cannot create private staging directory"
archive_copy="$stage/archive.tar.gz"
manifest_copy="$stage/oc2-release-manifest.json"
if ! cp -P "$ARCHIVE" "$archive_copy" || [ -L "$archive_copy" ] || [ ! -f "$archive_copy" ]; then
  integrity_fail "cannot stage regular archive"
fi
if ! cp -P "$MANIFEST" "$manifest_copy" || [ -L "$manifest_copy" ] || [ ! -f "$manifest_copy" ]; then
  integrity_fail "cannot stage regular manifest"
fi
if [ -L "$ARCHIVE" ] || [ ! -f "$ARCHIVE" ] || [ -L "$MANIFEST" ] || [ ! -f "$MANIFEST" ]; then
  integrity_fail "input changed type during staging"
fi
measure_file "$archive_copy" || integrity_fail "cannot measure staged archive"
if uint_gt "$FILE_SIZE" "$MAX_ARCHIVE_BYTES"; then
  integrity_fail "staged archive exceeds compressed byte limit"
fi
measure_file "$manifest_copy" || integrity_fail "cannot measure staged manifest"
if uint_gt "$FILE_SIZE" "$MAX_MANIFEST_BYTES"; then
  integrity_fail "staged manifest exceeds byte limit"
fi

actual_archive_sha256=$(sha256_file "$archive_copy") || integrity_fail "cannot hash archive"
[ "$actual_archive_sha256" = "$CHECKSUM" ] || integrity_fail "archive checksum mismatch"
manifest_values=$(parse_manifest "$manifest_copy") || integrity_fail "manifest is malformed or noncanonical"
[ "$manifest_values" = "" ] && integrity_fail "manifest parser returned no schema"

MANIFEST_ARCHIVE_SHA256=""
MANIFEST_BINARY_SHA256=""
MANIFEST_NATIVE_SHA256=""
MANIFEST_SBOM_SHA256=""
MANIFEST_PLATFORM=""
MANIFEST_REVISION=""
while IFS='=' read -r manifest_key manifest_value; do
  case "$manifest_key" in
    archive_sha256) MANIFEST_ARCHIVE_SHA256=$manifest_value ;;
    binary_sha256) MANIFEST_BINARY_SHA256=$manifest_value ;;
    native_sha256) MANIFEST_NATIVE_SHA256=$manifest_value ;;
    sbom_sha256) MANIFEST_SBOM_SHA256=$manifest_value ;;
    platform) MANIFEST_PLATFORM=$manifest_value ;;
    revision) MANIFEST_REVISION=$manifest_value ;;
    *) integrity_fail "manifest parser emitted an unknown field" ;;
  esac
done <<EOF_MANIFEST
$manifest_values
EOF_MANIFEST

is_lower_hex_64 "$MANIFEST_ARCHIVE_SHA256" || integrity_fail "invalid manifest archive digest"
is_lower_hex_64 "$MANIFEST_BINARY_SHA256" || integrity_fail "invalid binary member digest"
is_lower_hex_64 "$MANIFEST_NATIVE_SHA256" || integrity_fail "invalid native member digest"
is_lower_hex_64 "$MANIFEST_SBOM_SHA256" || integrity_fail "invalid SBOM member digest"
is_lower_hex_40 "$MANIFEST_REVISION" || integrity_fail "invalid manifest revision"
[ "$MANIFEST_PLATFORM" = "$PLATFORM" ] || integrity_fail "manifest platform mismatch"
[ "$MANIFEST_ARCHIVE_SHA256" = "$CHECKSUM" ] || integrity_fail "manifest/archive checksum mismatch"
[ "$MANIFEST_ARCHIVE_SHA256" = "$actual_archive_sha256" ] || integrity_fail "archive digest equality failed"

validate_gzip_header
decompress_bounded_archive

header_offset=0
probed_native_bytes=$(read_ustar_size "$header_offset") || integrity_fail "invalid native USTAR size"
if uint_gt "$probed_native_bytes" "$MAX_MEMBER_BYTES"; then
  integrity_fail "invalid or oversized native member"
fi
header_offset=$((header_offset + 512 + (probed_native_bytes + 511) / 512 * 512))
probed_binary_bytes=$(read_ustar_size "$header_offset") || integrity_fail "invalid binary USTAR size"
if uint_gt "$probed_binary_bytes" "$MAX_MEMBER_BYTES"; then
  integrity_fail "invalid or oversized $BIN member"
fi
header_offset=$((header_offset + 512 + (probed_binary_bytes + 511) / 512 * 512))
probed_sbom_bytes=$(read_ustar_size "$header_offset") || integrity_fail "invalid SBOM USTAR size"
if uint_gt "$probed_sbom_bytes" "$MAX_SBOM_BYTES"; then
  integrity_fail "invalid or oversized sbom.json"
fi
actual_total_bytes=$((probed_native_bytes + probed_binary_bytes + probed_sbom_bytes))
if uint_gt "$actual_total_bytes" "$MAX_TOTAL_MEMBER_BYTES"; then
  integrity_fail "expanded member payload exceeds aggregate limit"
fi

validate_ustar_headers

src="$stage/$BIN"
native_src="$stage/$EXPECTED_NATIVE"
sbom_src="$stage/sbom.json"
if ! mkdir -p "$stage/native/lib/$PLATFORM"; then
  integrity_fail "cannot create extraction staging directory"
fi
native_payload_offset=512
binary_payload_offset=$((native_payload_offset + (probed_native_bytes + 511) / 512 * 512 + 512))
sbom_payload_offset=$((binary_payload_offset + (probed_binary_bytes + 511) / 512 * 512 + 512))
if ! extract_member "$native_payload_offset" "$probed_native_bytes" "$native_src" ||
  ! extract_member "$binary_payload_offset" "$probed_binary_bytes" "$src" ||
  ! extract_member "$sbom_payload_offset" "$probed_sbom_bytes" "$sbom_src"; then
  integrity_fail "archive extraction failed"
fi
for staged_member in "$src" "$native_src" "$sbom_src"; do
  if [ -L "$staged_member" ] || [ ! -f "$staged_member" ]; then
    integrity_fail "extracted member is not a regular non-symlink file"
  fi
done

measure_file "$src" || integrity_fail "cannot measure staged binary"
actual_binary_bytes=$FILE_SIZE
measure_file "$native_src" || integrity_fail "cannot measure staged native"
actual_native_bytes=$FILE_SIZE
measure_file "$sbom_src" || integrity_fail "cannot measure staged SBOM"
actual_sbom_bytes=$FILE_SIZE
if uint_gt "$actual_binary_bytes" "$MAX_MEMBER_BYTES" || uint_gt "$actual_native_bytes" "$MAX_MEMBER_BYTES"; then
  integrity_fail "staged member exceeds per-member byte limit"
fi
if uint_gt "$actual_sbom_bytes" "$MAX_SBOM_BYTES"; then
  integrity_fail "SBOM exceeds $MAX_SBOM_BYTES bytes"
fi
actual_total_bytes=$((actual_binary_bytes + actual_native_bytes + actual_sbom_bytes))
if uint_gt "$actual_total_bytes" "$MAX_TOTAL_MEMBER_BYTES"; then
  integrity_fail "staged members exceed aggregate byte limit"
fi

actual_binary_sha256=$(sha256_file "$src") || integrity_fail "cannot hash staged binary"
actual_native_sha256=$(sha256_file "$native_src") || integrity_fail "cannot hash staged native"
actual_sbom_sha256=$(sha256_file "$sbom_src") || integrity_fail "cannot hash staged SBOM"
[ "$actual_binary_sha256" = "$MANIFEST_BINARY_SHA256" ] || integrity_fail "binary member hash mismatch"
[ "$actual_native_sha256" = "$MANIFEST_NATIVE_SHA256" ] || integrity_fail "native member hash mismatch"
[ "$actual_sbom_sha256" = "$MANIFEST_SBOM_SHA256" ] || integrity_fail "SBOM member hash mismatch"

# The release SBOM is bounded UTF-8 and must expose the SPDX 2.3 marker. This
# is intentionally a literal field check, not a claim of general JSON parsing.
iconv -f UTF-8 -t UTF-8 "$sbom_src" >/dev/null 2>&1 || integrity_fail "SBOM is not valid UTF-8"
grep -F '"spdxVersion":"SPDX-2.3"' "$sbom_src" >/dev/null 2>&1 || integrity_fail "SBOM lacks SPDX-2.3"

chmod 755 "$src" || identity_fail "cannot make staged binary executable"
if ! verify_runtime_receipt "$src"; then
  identity_fail "staged runtime receipt is missing, malformed, mismatched, or timed out"
fi
staged_receipt=$RUNTIME_LINE

# Recheck destination identity immediately before the first destination write.
if [ -L "$INSTALL_DIR" ] || [ -L "$native_dir" ]; then
  identity_fail "destination directory became a symlink"
fi
if [ -e "$INSTALL_DIR/opencode" ] || [ -L "$INSTALL_DIR/opencode" ]; then
  echo "refusing: $INSTALL_DIR/opencode exists; oc2 installs side-by-side only" >&2
  exit 73
fi
if ! mkdir -p "$INSTALL_DIR" "$native_dir"; then
  identity_fail "cannot create install directories"
fi
if [ -L "$INSTALL_DIR" ] || [ -L "$native_dir" ]; then
  identity_fail "destination directory became a symlink"
fi

binary_tmp=$(mktemp "$INSTALL_DIR/.oc2.tmp.XXXXXX" 2>/dev/null) || identity_fail "cannot stage installed binary"
native_tmp=$(mktemp "$native_dir/.libopentui.tmp.XXXXXX" 2>/dev/null) || identity_fail "cannot stage installed native library"
binary_backup=$(mktemp "$INSTALL_DIR/.oc2.backup.XXXXXX" 2>/dev/null) || identity_fail "cannot prepare binary rollback"
native_backup=$(mktemp "$native_dir/.libopentui.backup.XXXXXX" 2>/dev/null) || identity_fail "cannot prepare native rollback"
if ! cp "$src" "$binary_tmp" || ! chmod 755 "$binary_tmp"; then
  identity_fail "cannot stage installed binary"
fi
if ! cp "$native_src" "$native_tmp" || ! chmod 644 "$native_tmp"; then
  identity_fail "cannot stage installed native library"
fi

transaction_active=1
if [ -e "$target" ] || [ -L "$target" ]; then
  binary_backed=1
  if ! mv -f "$target" "$binary_backup"; then
    binary_backed=0
    rollback_install 74
  fi
fi
if [ -e "$native_target" ] || [ -L "$native_target" ]; then
  native_backed=1
  if ! mv -f "$native_target" "$native_backup"; then
    native_backed=0
    rollback_install 74
  fi
fi
if ! mv -f "$binary_tmp" "$target"; then
  rollback_install 74
fi
binary_installed=1
if ! mv -f "$native_tmp" "$native_target"; then
  rollback_install 74
fi
native_installed=1

# Keep the transaction live through installed observation. Any receipt failure
# removes new files and restores both old bytes.
if ! verify_runtime_receipt "$target"; then
  rollback_install 74
fi

transaction_active=0
rm -f "$binary_backup" "$native_backup" 2>/dev/null || :
echo "installed $target ($PLATFORM${VERSION:+ version: $VERSION})" >&2
printf '%s\n' "$staged_receipt"
