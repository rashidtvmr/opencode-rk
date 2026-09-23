#!/usr/bin/env bash
# build_opentui.sh - fail-closed macOS arm64 builder for the pinned OpenTUI fork.
#
# Pins (hardcoded, never HEAD/latest):
#   fork repo    https://github.com/rashidtvmr/opentui.git
#   fork commit  c01292fd0837bafd07ce458c74416b2b375a41ab
#   fork tree    261e8ea4b0ac68bb589760c3c871e202ef71ed6b
#   zig version  0.16.0 (packages/native/build.zig.zon minimum_zig_version,
#                build.zig SUPPORTED_ZIG_VERSIONS exact match)
#   zig archive  https://ziglang.org/download/0.16.0/zig-aarch64-macos-0.16.0.tar.xz
#   zig sha256   b23d70deaa879b5c2d486ed3316f7eaa53e84acf6fc9cc747de152450d401489
#   zig target   aarch64-macos.13.0 (build.zig SUPPORTED_TARGETS)
#   build iface  zig build -Dlibrary-target=<t> -Doptimize=ReleaseSafe [-Dmacos-sdk=...]
#                run in <checkout>/packages/native; artifact lands in
#                packages/native/lib/aarch64-macos/libopentui.dylib
#
# Modes:
#   build_opentui.sh --work-dir <abs> --output-dir <abs>
#   build_opentui.sh --verify-only <artifact>
#   build_opentui.sh --help
#
# Exit codes: 0 ok; 1 build/verify failure; 2 usage/argument error.
# Stdout carries only the final artifact SHA-256 line. Diagnostics go to stderr.
# No secrets are printed, forwarded, or persisted.

set -euo pipefail

# ---------------------------------------------------------------- pins ------
readonly FORK_REPO="https://github.com/rashidtvmr/opentui.git"
readonly FORK_COMMIT="c01292fd0837bafd07ce458c74416b2b375a41ab"
readonly FORK_TREE="261e8ea4b0ac68bb589760c3c871e202ef71ed6b"
readonly ZIG_VERSION="0.16.0"
readonly ZIG_TARBALL_URL="https://ziglang.org/download/0.16.0/zig-aarch64-macos-0.16.0.tar.xz"
readonly ZIG_TARBALL_SHA256="b23d70deaa879b5c2d486ed3316f7eaa53e84acf6fc9cc747de152450d401489"
readonly ZIG_TARBALL_SIZE="52238004"
readonly ZIG_TARGET="aarch64-macos.13.0"
readonly ZIG_OUTPUT_NAME="aarch64-macos"
readonly ZIG_BUILD_SUBDIR="packages/native"
readonly ZIG_PREPARE_SCRIPT="scripts/prepare-zig-deps.sh"
readonly EXPECTED_LIB="libopentui.dylib"
readonly OPTIMIZE_MODE="ReleaseSafe"

# Required exported ABI symbols (nm -gU names carry a leading underscore).
readonly REQUIRED_SYMBOLS="createRenderer destroyRenderer getCurrentBuffer bufferDrawText bufferWriteResolvedChars"

# -------------------------------------------------------------- bounds ------
readonly MAX_ZIG_ARCHIVE_BYTES=134217728      # 128 MiB
readonly MAX_ZIG_EXTRACTED_BYTES=1073741824   # 1 GiB
readonly MAX_SOURCE_BYTES=1073741824          # 1 GiB
readonly MAX_ARTIFACT_BYTES=134217728        # 128 MiB
readonly CURL_MAX_TIME=300
readonly CURL_MAX_REDIRS=5

# ------------------------------------------------------ secret hygiene ------
# Never forward inherited credentials to git/curl/zig. Public HTTPS needs none.
unset AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY AWS_SESSION_TOKEN 2>/dev/null || true
unset GH_TOKEN GITHUB_TOKEN GITHUB_OAUTH NPM_TOKEN BUN_AUTH_TOKEN 2>/dev/null || true
unset CARGO_REGISTRIES_CRATES_IO_PROTOCOL 2>/dev/null || true

die() { printf 'build_opentui: error: %s\n' "$*" >&2; exit 1; }
usage_die() { printf 'build_opentui: error: %s\n' "$*" >&2; print_help >&2; exit 2; }

print_help() {
  cat <<'EOF'
Usage:
  build_opentui.sh --work-dir <abs-dir> --output-dir <abs-dir>
  build_opentui.sh --verify-only <artifact-path>
  build_opentui.sh --help

Builds the pinned OpenTUI fork (rashidtvmr/opentui@c01292f, Zig 0.16.0)
packages/native shared library for aarch64-macos (ReleaseSafe) and copies
libopentui.dylib atomically into <output-dir>. Prints the artifact SHA-256
on stdout; all diagnostics go to stderr.

  --work-dir    caller-supplied absolute scratch dir (created if missing).
                Must not live inside this repository.
  --output-dir  caller-supplied absolute dir receiving libopentui.dylib.
                Must not live inside this repository.
  --verify-only verify an existing artifact (Mach-O arm64, ABI symbols,
                rpath/install-name safety) and print its SHA-256.
  --help        print this text and exit 0.

Pins: fork commit c01292fd0837bafd07ce458c74416b2b375a41ab, tree
261e8ea4b0ac68bb589760c3c871e202ef71ed6b, Zig 0.16.0 tarball
https://ziglang.org/download/0.16.0/zig-aarch64-macos-0.16.0.tar.xz
sha256 b23d70deaa879b5c2d486ed3316f7eaa53e84acf6fc9cc747de152450d401489.
HTTPS only; no sudo, no package-manager mutation, no npm/Bun lifecycle.
Exit codes: 0 ok, 1 build/verify failure, 2 usage error.
EOF
}

# Repo root of this script (used only to refuse work/output dirs inside it).
script_repo_root() {
  local d
  d="$(CDPATH= cd -- "$(dirname -- "$0")" >/dev/null 2>&1 && pwd -P)" || return 0
  while [ "$d" != "/" ]; do
    if [ -d "$d/.git" ]; then printf '%s' "$d"; return 0; fi
    d="$(dirname -- "$d")"
  done
  return 0
}

require_abs_dir_outside_repo() {
  local what="$1" dir="$2" repo
  case "$dir" in
    /*) ;;
    *) usage_die "$what must be an absolute path, got: $dir" ;;
  esac
  case "$dir" in
    /|/tmp|/private/tmp) usage_die "$what must not be a system root/tmp: $dir" ;;
  esac
  repo="$(script_repo_root)"
  if [ -n "$repo" ]; then
    case "$dir" in
      "$repo"/*|"$repo") usage_die "$what must be caller-supplied outside this repo: $dir" ;;
    esac
  fi
}

dir_bytes() {
  # du -sk is POSIX on macOS; paths here are internal temp dirs, no "--".
  local d="$1" kb
  kb="$(du -sk "$d" 2>/dev/null | awk '{print $1}')" || die "cannot size $d"
  printf '%s' "$((kb * 1024))"
}

file_bytes() {
  wc -c < "$1" | tr -d '[:space:]'
}

sha256_of() {
  # shasum path is caller/builder controlled; single positional arg.
  shasum -a 256 "$1" | awk '{print $1}'
}

# ------------------------------------------------------------ verify -------
verify_artifact() {
  local art="$1"
  [ -f "$art" ] || die "artifact not found: $art"
  local size
  size="$(file_bytes "$art")"
  [ "$size" -gt 0 ] || die "artifact is empty: $art"
  [ "$size" -le "$MAX_ARTIFACT_BYTES" ] || die "artifact $size bytes exceeds bound $MAX_ARTIFACT_BYTES"

  # Mach-O 64-bit arm64 shared library, via file(1) ground truth.
  local ftype
  ftype="$(file -b "$art")" || die "file(1) failed on artifact"
  case "$ftype" in
    *"Mach-O 64-bit dynamically linked shared library arm64"*) ;;
    *) die "not a Mach-O 64-bit arm64 dylib: $ftype" ;;
  esac
  printf 'build_opentui: file type ok: %s\n' "$ftype" >&2

  # Architecture confirmation via lipo when available.
  if command -v lipo >/dev/null 2>&1; then
    local archs
    archs="$(lipo -archs "$art")" || die "lipo failed on artifact"
    [ "$archs" = "arm64" ] || die "lipo archs '$archs' is not exactly 'arm64'"
    printf 'build_opentui: lipo archs ok: %s\n' "$archs" >&2
  else
    printf 'build_opentui: warning: lipo absent, file(1) only\n' >&2
  fi

  # Required exported ABI symbols. Pipe-free match: grep -q in a pipeline
  # under pipefail/SIGPIPE false-negatives on large nm output.
  local nm_out sym missing=0 nl padded
  nm_out="$(nm -gU "$art" 2>/dev/null)" || die "nm -gU failed on artifact"
  nl="$(printf '\n_')"
  nl="${nl%_}"
  padded="$nl$nm_out$nl"
  for sym in $REQUIRED_SYMBOLS; do
    case "$padded" in
      *" _${sym}${nl}"*) ;;
      *)
        printf 'build_opentui: error: missing exported symbol: %s\n' "$sym" >&2
        missing=1 ;;
    esac
  done
  [ "$missing" -eq 0 ] || die "required ABI symbols absent"
  printf 'build_opentui: ABI symbols ok: %s\n' "$REQUIRED_SYMBOLS" >&2

  # Reject unsafe absolute rpaths: temp/build-tree leakage or parent escapes.
  local rpaths rpath bad=0
  rpaths="$(otool -l "$art" 2>/dev/null | awk '/^ *cmd LC_RPATH/{f=1;next} /^ *cmd /{f=0} f==1 && /^ *path /{print $2}')" \
    || die "otool -l failed on artifact"
  if [ -n "$rpaths" ]; then
    while IFS= read -r rpath; do
      [ -n "$rpath" ] || continue
      case "$rpath" in
        @loader_path*|@rpath*|@executable_path*)
          printf 'build_opentui: rpath ok (relative): %s\n' "$rpath" >&2 ;;
        /tmp/*|/private/*|*/".."*|*"\$"*|*" "*)
          printf 'build_opentui: error: unsafe absolute rpath: %s\n' "$rpath" >&2
          bad=1 ;;
        /*)
          printf 'build_opentui: error: absolute rpath not allowed for vendored dylib: %s\n' "$rpath" >&2
          bad=1 ;;
        *) printf 'build_opentui: error: unrecognized rpath: %s\n' "$rpath" >&2; bad=1 ;;
      esac
    done <<EOF_RPATHS
$rpaths
EOF_RPATHS
  else
    printf 'build_opentui: no LC_RPATH entries (ok)\n' >&2
  fi
  [ "$bad" -eq 0 ] || die "unsafe rpaths rejected"

  # Install name: must not leak a build-temp absolute path.
  local install_name
  install_name="$(otool -D "$art" 2>/dev/null | tail -n +2 | head -n 1)" \
    || die "otool -D failed on artifact"
  [ -n "$install_name" ] || die "empty install name"
  case "$install_name" in
    /tmp/*|/private/*|*/".."*|*"\$"*|*" "*)
      die "unsafe absolute install name rejected: $install_name" ;;
  esac
  printf 'build_opentui: install name ok: %s\n' "$install_name" >&2

  sha256_of "$art"
}

# ---------------------------------------------------------------- main ------
MODE="build"
VERIFY_PATH=""
WORK_DIR=""
OUTPUT_DIR=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --help|-h) print_help; exit 0 ;;
    --verify-only)
      [ "$#" -ge 2 ] || usage_die "--verify-only needs an artifact path"
      MODE="verify"; VERIFY_PATH="$2"; shift 2 ;;
    --work-dir)
      [ "$#" -ge 2 ] || usage_die "--work-dir needs a directory"
      WORK_DIR="$2"; shift 2 ;;
    --output-dir)
      [ "$#" -ge 2 ] || usage_die "--output-dir needs a directory"
      OUTPUT_DIR="$2"; shift 2 ;;
    --) shift; break ;;
    -*) usage_die "unknown flag: $1" ;;
    *) usage_die "unexpected argument: $1" ;;
  esac
done

if [ "$MODE" = "verify" ]; then
  [ -n "$WORK_DIR$OUTPUT_DIR" ] && [ -n "$WORK_DIR$OUTPUT_DIR" ] && {
    [ -n "$WORK_DIR" ] || [ -n "$OUTPUT_DIR" ] && usage_die "--verify-only takes no --work-dir/--output-dir"
  }
  [ -z "$VERIFY_PATH" ] && usage_die "--verify-only needs an artifact path"
  sum="$(verify_artifact "$VERIFY_PATH")"
  printf '%s  %s\n' "$sum" "$VERIFY_PATH"
  exit 0
fi

[ -n "$WORK_DIR" ] || usage_die "--work-dir is required"
[ -n "$OUTPUT_DIR" ] || usage_die "--output-dir is required"
require_abs_dir_outside_repo "--work-dir" "$WORK_DIR"
require_abs_dir_outside_repo "--output-dir" "$OUTPUT_DIR"

for tool in curl git tar shasum file nm otool sh cksum awk; do
  command -v "$tool" >/dev/null 2>&1 || die "required tool missing: $tool"
done
[ "$(uname -s)" = "Darwin" ] || die "this builder supports macOS only"
[ "$(uname -m)" = "arm64" ] || die "this builder supports arm64 only"

mkdir_args=(mkdir -p -- "$WORK_DIR" "$OUTPUT_DIR")
"${mkdir_args[@]}" || die "cannot create work/output dirs"

# Temp area lives under the caller work dir; cleaned by trap.
TMP_ROOT="$(mktemp -d -- "$WORK_DIR/build-tmp.XXXXXX")" || die "mktemp failed"
cleanup() { rm -rf -- "$TMP_ROOT"; }
trap cleanup EXIT HUP INT TERM

SRC_DIR="$TMP_ROOT/opentui-src"
ZIG_DL="$TMP_ROOT/zig.tar.xz"
ZIG_ROOT="$TMP_ROOT/zig-dist"

# ------------------------------------------------------- 1. fetch source ---
printf 'build_opentui: fetching pinned fork commit\n' >&2
git_init_args=(git init -q -- "$SRC_DIR")
"${git_init_args[@]}" || die "git init failed"
git_remote_args=(git -C "$SRC_DIR" remote add origin "$FORK_REPO")
"${git_remote_args[@]}" || die "git remote add failed"
# Fail closed if the remote URL is anything but the pinned HTTPS origin.
actual_url="$(git -C "$SRC_DIR" remote get-url origin)" || die "git remote get-url failed"
[ "$actual_url" = "$FORK_REPO" ] || die "remote URL mismatch: $actual_url"
case "$actual_url" in
  https://*) ;;
  *) die "non-HTTPS remote refused: $actual_url" ;;
esac
git_fetch_args=(git -C "$SRC_DIR" fetch --depth 1 --no-tags --no-recurse-submodules origin "$FORK_COMMIT")
"${git_fetch_args[@]}" || die "git fetch of pinned commit failed"
git_checkout_args=(git -C "$SRC_DIR" checkout -q --detach FETCH_HEAD)
"${git_checkout_args[@]}" || die "git checkout failed"

got_commit="$(git -C "$SRC_DIR" rev-parse HEAD)" || die "rev-parse HEAD failed"
[ "$got_commit" = "$FORK_COMMIT" ] || die "commit mismatch: $got_commit"
got_tree="$(git -C "$SRC_DIR" rev-parse 'HEAD^{tree}')" || die "rev-parse tree failed"
[ "$got_tree" = "$FORK_TREE" ] || die "source tree mismatch: $got_tree"
printf 'build_opentui: source ok: commit %s tree %s\n' "$got_commit" "$got_tree" >&2

src_bytes="$(dir_bytes "$SRC_DIR")"
[ "$src_bytes" -le "$MAX_SOURCE_BYTES" ] || die "source $src_bytes bytes exceeds bound"
[ -f "$SRC_DIR/$ZIG_BUILD_SUBDIR/build.zig" ] || die "fork build interface missing: $ZIG_BUILD_SUBDIR/build.zig"
[ -f "$SRC_DIR/$ZIG_BUILD_SUBDIR/build.zig.zon" ] || die "fork build metadata missing: build.zig.zon"

# ---------------------------------------------------------- 2. fetch zig ---
printf 'build_opentui: downloading pinned Zig %s\n' "$ZIG_VERSION" >&2
curl_args=(curl --proto "=https" --tlsv1.2 --fail --silent --show-error --location --max-redirs "$CURL_MAX_REDIRS" --max-time "$CURL_MAX_TIME" --retry 0 --output "$ZIG_DL" --url "$ZIG_TARBALL_URL")
"${curl_args[@]}" || die "zig download failed"
case "$ZIG_TARBALL_URL" in
  https://*) ;;
  *) die "non-HTTPS zig URL refused" ;;
esac
dl_bytes="$(file_bytes "$ZIG_DL")"
[ "$dl_bytes" -le "$MAX_ZIG_ARCHIVE_BYTES" ] || die "zig archive $dl_bytes bytes exceeds bound"
[ "$dl_bytes" = "$ZIG_TARBALL_SIZE" ] || die "zig archive size $dl_bytes != pinned $ZIG_TARBALL_SIZE"
got_zig_sha="$(sha256_of "$ZIG_DL")"
[ "$got_zig_sha" = "$ZIG_TARBALL_SHA256" ] || die "zig archive SHA-256 mismatch"
printf 'build_opentui: zig archive sha ok\n' >&2

mkdir -p -- "$ZIG_ROOT" || die "cannot create zig dest"
tar_args=(tar -xf "$ZIG_DL" -C "$ZIG_ROOT")
"${tar_args[@]}" || die "zig archive extract failed"
zig_bytes="$(dir_bytes "$ZIG_ROOT")"
[ "$zig_bytes" -le "$MAX_ZIG_EXTRACTED_BYTES" ] || die "zig extracted $zig_bytes bytes exceeds bound"
ZIG_BIN="$ZIG_ROOT/zig-aarch64-macos-$ZIG_VERSION/zig"
[ -x "$ZIG_BIN" ] || die "zig binary missing after extract"
got_zig_ver="$("$ZIG_BIN" version)" || die "zig version failed"
[ "$got_zig_ver" = "$ZIG_VERSION" ] || die "zig version '$got_zig_ver' != pinned '$ZIG_VERSION'"
printf 'build_opentui: zig ok: %s\n' "$got_zig_ver" >&2

# ------------------------------------------------------ 3. prepare deps ----
printf 'build_opentui: preparing vendored zig-deps\n' >&2
[ -f "$SRC_DIR/$ZIG_BUILD_SUBDIR/$ZIG_PREPARE_SCRIPT" ] || die "prepare script missing"
sh_prepare_args=(sh "$ZIG_PREPARE_SCRIPT")
(cd -- "$SRC_DIR/$ZIG_BUILD_SUBDIR" && "${sh_prepare_args[@]}") || die "zig-deps prepare failed"
[ -d "$SRC_DIR/$ZIG_BUILD_SUBDIR/zig-deps" ] || die "zig-deps dir missing after prepare"

# -------------------------------------------------------------- 4. build ---
# macOS SDK is mandatory for the aarch64-macos target (CoreAudio/AppKit).
MACOS_SDK=""
for candidate in "${SDKROOT:-}" "${MACOS_SDK_PATH:-}" "${MACOSX_SDK_PATH:-}" \
    /Library/Developer/CommandLineTools/SDKs/MacOSX.sdk; do
  if [ -n "$candidate" ] && [ -d "$candidate/System/Library/Frameworks/CoreFoundation.framework" ]; then
    MACOS_SDK="$candidate"
    break
  fi
done
[ -n "$MACOS_SDK" ] || die "macOS SDK with CoreFoundation.framework not found; set SDKROOT"

printf 'build_opentui: building %s (%s)\n' "$ZIG_TARGET" "$OPTIMIZE_MODE" >&2
zig_build_args=("$ZIG_BIN" build "-Dlibrary-target=$ZIG_TARGET" "-Doptimize=$OPTIMIZE_MODE" "-Dmacos-sdk=$MACOS_SDK")
(cd -- "$SRC_DIR/$ZIG_BUILD_SUBDIR" && "${zig_build_args[@]}") || die "zig build failed"

BUILT_LIB="$SRC_DIR/$ZIG_BUILD_SUBDIR/lib/$ZIG_OUTPUT_NAME/$EXPECTED_LIB"
[ -f "$BUILT_LIB" ] || die "expected build output missing: $BUILT_LIB"

# ------------------------------------------------------------ 5. verify ----
printf 'build_opentui: verifying built artifact\n' >&2
built_sum="$(verify_artifact "$BUILT_LIB")"
printf 'build_opentui: built artifact sha: %s\n' "$built_sum" >&2

# -------------------------------------------------------------- 6. copy ----
# Atomic publish: copy to temp file in output dir, then rename.
STAGED="$OUTPUT_DIR/.$EXPECTED_LIB.tmp.$$"
cp_args=(cp -- "$BUILT_LIB" "$STAGED")
"${cp_args[@]}" || die "stage copy failed"
mv_args=(mv -f -- "$STAGED" "$OUTPUT_DIR/$EXPECTED_LIB")
"${mv_args[@]}" || die "atomic publish failed"
trap - EXIT HUP INT TERM
cleanup

final_sum="$(sha256_of "$OUTPUT_DIR/$EXPECTED_LIB")"
[ "$final_sum" = "$built_sum" ] || die "published artifact hash changed during copy"
printf '%s  %s\n' "$final_sum" "$OUTPUT_DIR/$EXPECTED_LIB"
