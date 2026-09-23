#!/usr/bin/env bash
# build_opentui.sh - fail-closed pinned builder for the OpenTUI fork, Rust-target matrix.
#
# Pins (hardcoded, never HEAD/latest):
#   fork repo    https://github.com/rashidtvmr/opentui.git
#   fork commit  c01292fd0837bafd07ce458c74416b2b375a41ab
#   fork tree    261e8ea4b0ac68bb589760c3c871e202ef71ed6b
#   zig version  0.16.0 (packages/native/build.zig.zon minimum_zig_version,
#                build.zig SUPPORTED_ZIG_VERSIONS exact match)
#   zig archive  https://ziglang.org/download/0.16.0/zig-aarch64-macos-0.16.0.tar.xz
#                (Darwin/arm64 host only; every other host fails closed, no
#                fabricated per-host hashes)
#   zig sha256   b23d70deaa879b5c2d486ed3316f7eaa53e84acf6fc9cc747de152450d401489
#   build iface  zig build -Dlibrary-target=<t> -Doptimize=ReleaseSafe [-Dmacos-sdk=...]
#                [--static adds the hash-pinned -Dlinkage=static fork patch]
#                run in <checkout>/packages/native; artifacts land in
#                packages/native/lib/<zig-output-name>/
#
# Rust-target matrix (rust triple -> zig target -> zig output dir -> checks):
#   aarch64-apple-darwin      aarch64-macos.13.0      aarch64-macos    Mach-O arm64  libopentui.dylib
#   x86_64-apple-darwin       x86_64-macos.13.0       x86_64-macos     Mach-O x86_64 libopentui.dylib
#   aarch64-unknown-linux-gnu aarch64-linux-gnu.2.17  aarch64-linux    ELF aarch64   libopentui.so
#   x86_64-unknown-linux-gnu  x86_64-linux-gnu.2.17   x86_64-linux     ELF x86_64    libopentui.so
#   x86_64-pc-windows-gnu     x86_64-windows-gnu      x86_64-windows   PE64 x86_64   opentui.dll + libopentui.dll.a
#                             (import lib derived from the verified DLL export
#                             table via pinned zig dlltool; zig build installs
#                             only DLL+PDB into lib/<out>, plus its own
#                             zig-out/lib/opentui.lib ar archive)
#   x86_64-pc-windows-msvc    UNSUPPORTED: pinned build.zig SUPPORTED_TARGETS
#                             lists only *-windows-gnu; no MSVC ABI target exists,
#                             so this builder refuses to invent one (exit 1).
#
# Modes:
#   build_opentui.sh --work-dir <abs> --output-dir <abs> [--target <rust-triple>]
#                     [--static]
#   build_opentui.sh --verify-only <artifact> [--target <rust-triple>]
#   build_opentui.sh --help
#
# Exit codes: 0 ok; 1 build/verify failure (incl. truthfully unproducible
# target/tool combo); 2 usage/argument error (incl. unknown --target).
# Stdout carries only the final artifact SHA-256 line(s) (two lines for the
# Windows DLL+import-lib pair). Diagnostics go to stderr.
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
readonly ZIG_BUILD_SUBDIR="packages/native"
readonly ZIG_PREPARE_SCRIPT="scripts/prepare-zig-deps.sh"
readonly OPTIMIZE_MODE="ReleaseSafe"
readonly DEFAULT_RUST_TARGET="aarch64-apple-darwin"
readonly STATIC_PATCH_SHA256="2da616bc1e71a8229f9b392fc32fc6ead651aabfbe9194f0828554e71a16d500"
readonly STATIC_BUILD_ZIG_PREIMAGE_SHA256="8e7e25080db07d7c333e54258f48fd453676e522154f780d0aa6770e86cf8b0e"
readonly STATIC_BUILD_ZIG_POSTIMAGE_SHA256="67993a110f474a58f956fb9cd3efc020f332b651663ee1bb62b966b21fb717f7"

# Required exported ABI symbols (Mach-O nm -gU names carry a leading
# underscore; ELF nm -D and the PE export table use undecorated names).
readonly REQUIRED_SYMBOLS="createRenderer destroyRenderer getCurrentBuffer bufferDrawText bufferWriteResolvedChars"

# ELF DT_NEEDED allowlist: bare sonames only, no paths. glibc commercial
# floor is 2.17 per the pinned zig targets (aarch64/x86_64-linux-gnu.2.17).
readonly ELF_ALLOWED_NEEDED="ld-linux-x86-64.so.2 ld-linux-aarch64.so.1 libm.so.6 libc.so.6 libpthread.so.0 libdl.so.2 librt.so.1"

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
  build_opentui.sh --work-dir <abs-dir> --output-dir <abs-dir> [--target <rust-triple>] [--static]
  build_opentui.sh --verify-only <artifact-path> [--target <rust-triple>]
  build_opentui.sh --help

Builds the pinned OpenTUI fork (rashidtvmr/opentui@c01292f, Zig 0.16.0)
packages/native shared library for --target (default aarch64-apple-darwin,
ReleaseSafe) and copies the exact build.rs filename(s) atomically into
<output-dir>. Prints the artifact SHA-256 on stdout (two lines for the
Windows DLL+import-lib pair); all diagnostics go to stderr.

Supported --target values (rust triple -> zig -Dlibrary-target):
  aarch64-apple-darwin      -> aarch64-macos.13.0     (libopentui.dylib)
  x86_64-apple-darwin       -> x86_64-macos.13.0      (libopentui.dylib)
  aarch64-unknown-linux-gnu -> aarch64-linux-gnu.2.17 (libopentui.so)
  x86_64-unknown-linux-gnu  -> x86_64-linux-gnu.2.17  (libopentui.so)
  x86_64-pc-windows-gnu     -> x86_64-windows-gnu     (opentui.dll + libopentui.dll.a,
                               import lib generated from the verified DLL
                               export table with pinned zig dlltool)
  x86_64-pc-windows-msvc    -> unsupported by the pinned interface
                               (SUPPORTED_TARGETS has no MSVC ABI entry);
                               refused with exit 1, never faked.

  --work-dir    caller-supplied absolute scratch dir (created if missing).
                Must not live inside this repository.
  --output-dir  caller-supplied absolute dir receiving the artifact file(s).
                Must not live inside this repository.
  --verify-only verify an existing artifact for --target (Mach-O arm64/x86_64
                 + ABI + rpath/install-name; ELF arch + ABI + NEEDED/RPATH;
                 PE64 DLL + import lib + export table) and print its SHA-256.
  --static      build the aarch64 macOS static archive as
                libopentui_static.a. The pinned fork patch, source pre/post
                hashes, archive members, and complete exported ABI are checked.
  --help        print this text and exit 0.

Host: Darwin/arm64 only (the sole pinned Zig archive hash covers
zig-aarch64-macos). Any other host fails closed with exit 1.
Pins: fork commit c01292fd0837bafd07ce458c74416b2b375a41ab, tree
261e8ea4b0ac68bb589760c3c871e202ef71ed6b, Zig 0.16.0 tarball
https://ziglang.org/download/0.16.0/zig-aarch64-macos-0.16.0.tar.xz
sha256 b23d70deaa879b5c2d486ed3316f7eaa53e84acf6fc9cc747de152450d401489.
HTTPS only; no sudo, no package-manager mutation, no npm/Bun lifecycle.
Exit codes: 0 ok, 1 build/verify failure, 2 usage error.
EOF
}

# ------------------------------------------------------- target table ------
# kind: macho | elf | pe | unsupported-msvc
target_kind() {
  case "$1" in
    aarch64-apple-darwin) printf 'macho' ;;
    x86_64-apple-darwin) printf 'macho' ;;
    aarch64-unknown-linux-gnu) printf 'elf' ;;
    x86_64-unknown-linux-gnu) printf 'elf' ;;
    x86_64-pc-windows-gnu) printf 'pe' ;;
    x86_64-pc-windows-msvc) printf 'unsupported-msvc' ;;
    *) printf 'unknown' ;;
  esac
}

target_zig() {
  case "$1" in
    aarch64-apple-darwin) printf 'aarch64-macos.13.0' ;;
    x86_64-apple-darwin) printf 'x86_64-macos.13.0' ;;
    aarch64-unknown-linux-gnu) printf 'aarch64-linux-gnu.2.17' ;;
    x86_64-unknown-linux-gnu) printf 'x86_64-linux-gnu.2.17' ;;
    x86_64-pc-windows-gnu) printf 'x86_64-windows-gnu' ;;
    *) printf '' ;;
  esac
}

# Zig build.zig output dir name under packages/native/lib/.
target_outname() {
  case "$1" in
    aarch64-apple-darwin) printf 'aarch64-macos' ;;
    x86_64-apple-darwin) printf 'x86_64-macos' ;;
    aarch64-unknown-linux-gnu) printf 'aarch64-linux' ;;
    x86_64-unknown-linux-gnu) printf 'x86_64-linux' ;;
    x86_64-pc-windows-gnu) printf 'x86_64-windows' ;;
    *) printf '' ;;
  esac
}

# Repo/output artifact filenames, exactly as crates/opentui-bridge/build.rs expects.
target_dll_name() {
  if [ "${LINKAGE:-dynamic}" = "static" ]; then
    case "$1" in
      aarch64-apple-darwin) printf 'libopentui_static.a'; return ;;
      *) printf ''; return ;;
    esac
  fi
  case "$1" in
    *-apple-darwin) printf 'libopentui.dylib' ;;
    *-unknown-linux-gnu) printf 'libopentui.so' ;;
    x86_64-pc-windows-gnu) printf 'opentui.dll' ;;
    *) printf '' ;;
  esac
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
verify_macho() {
  local art="$1" expect_arch="$2" expect_label="$3"
  local size
  size="$(file_bytes "$art")"
  [ "$size" -gt 0 ] || die "artifact is empty: $art"
  [ "$size" -le "$MAX_ARTIFACT_BYTES" ] || die "artifact $size bytes exceeds bound $MAX_ARTIFACT_BYTES"

  # Mach-O 64-bit shared library for the expected arch, via file(1).
  local ftype
  ftype="$(file -b "$art")" || die "file(1) failed on artifact"
  case "$ftype" in
    *"Mach-O 64-bit dynamically linked shared library $expect_label"*) ;;
    *) die "not a Mach-O 64-bit $expect_arch dylib: $ftype" ;;
  esac
  printf 'build_opentui: file type ok: %s\n' "$ftype" >&2

  # Architecture confirmation via lipo when available.
  if command -v lipo >/dev/null 2>&1; then
    local archs
    archs="$(lipo -archs "$art")" || die "lipo failed on artifact"
    [ "$archs" = "$expect_arch" ] || die "lipo archs '$archs' is not exactly '$expect_arch'"
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

verify_static_macho() {
  local art="$1" size ftype members member count=0 check_dir sym nm_out nl padded missing=0
  art="$(CDPATH= cd -- "$(dirname -- "$art")" && pwd -P)/$(basename -- "$art")" \
    || die "cannot resolve static artifact path"
  size="$(file_bytes "$art")"
  [ "$size" -gt 0 ] || die "static artifact is empty: $art"
  [ "$size" -le "$MAX_ARTIFACT_BYTES" ] || die "static artifact $size bytes exceeds bound $MAX_ARTIFACT_BYTES"
  ftype="$(file -b "$art")" || die "file(1) failed on static artifact"
  case "$ftype" in
    *"ar archive"*) ;;
    *) die "not an ar static archive: $ftype" ;;
  esac

  members="$(ar -t "$art")" || die "ar listing failed"
  [ -n "$members" ] || die "static archive has no members"
  while IFS= read -r member; do
    [ -n "$member" ] || continue
    case "$member" in
      __.SYMDEF|*.o) ;;
      *) die "unsafe or unexpected static member: $member" ;;
    esac
    case "$member" in */*|..|.*.o) die "unsafe static member path: $member" ;; esac
    count=$((count + 1))
    [ "$count" -le 512 ] || die "static archive member bound exceeded"
  done <<EOF_STATIC_MEMBERS
$members
EOF_STATIC_MEMBERS
  [ "$count" -ge 2 ] || die "static archive is incomplete"

  check_dir="$(mktemp -d "${TMPDIR:-/tmp}/opentui-static-verify.XXXXXX")" || die "static verify mktemp failed"
  (cd -- "$check_dir" && ar -x "$art") || { rm -rf -- "$check_dir"; die "static archive extraction failed"; }
  [ "$(dir_bytes "$check_dir")" -le "$MAX_ARTIFACT_BYTES" ] || { rm -rf -- "$check_dir"; die "static members exceed artifact bound"; }
  for member in "$check_dir"/*.o; do
    [ -f "$member" ] || { rm -rf -- "$check_dir"; die "static archive has no object members"; }
    ftype="$(file -b "$member")" || { rm -rf -- "$check_dir"; die "file(1) failed on static member"; }
    case "$ftype" in
      *"Mach-O 64-bit object arm64"*) ;;
      *) rm -rf -- "$check_dir"; die "non-arm64 Mach-O static member: $ftype" ;;
    esac
  done
  rm -rf -- "$check_dir"

  nm_out="$(nm -gU "$art" 2>/dev/null)" || die "nm -gU failed on static artifact"
  nl="$(printf '\n_')"; nl="${nl%_}"; padded="$nl$nm_out$nl"
  for sym in $REQUIRED_SYMBOLS; do
    case "$padded" in *" _${sym}${nl}"*) ;; *) missing=1; printf 'build_opentui: error: missing static symbol: %s\n' "$sym" >&2 ;; esac
  done
  [ "$missing" -eq 0 ] || die "required static ABI symbols absent"
  printf 'build_opentui: static archive ok: %s members, arm64 Mach-O, required ABI present\n' "$count" >&2
  sha256_of "$art"
}

verify_static_exports_from_source() {
  local art="$1" source="$2" expected sym nm_out nl padded missing=0 count
  expected="$(sed -n 's/^[[:space:]]*export fn \([A-Za-z_][A-Za-z0-9_]*\).*/\1/p' "$source" | LC_ALL=C sort -u)" \
    || die "cannot derive pinned exported ABI"
  count="$(printf '%s\n' "$expected" | sed '/^$/d' | wc -l | tr -d '[:space:]')"
  [ "$count" = "369" ] || die "pinned exported ABI count drifted: $count != 369"
  nm_out="$(nm -gU "$art" 2>/dev/null)" || die "nm failed during complete static ABI check"
  nl="$(printf '\n_')"; nl="${nl%_}"; padded="$nl$nm_out$nl"
  while IFS= read -r sym; do
    [ -n "$sym" ] || continue
    case "$padded" in *" _${sym}${nl}"*) ;; *) missing=1; printf 'build_opentui: error: static ABI missing: %s\n' "$sym" >&2 ;; esac
  done <<EOF_STATIC_EXPORTS
$expected
EOF_STATIC_EXPORTS
  [ "$missing" -eq 0 ] || die "static archive does not contain the complete pinned ABI"
  printf 'build_opentui: complete static ABI ok: %s/%s exports\n' "$count" "$count" >&2
}

verify_elf() {
  local art="$1" expect_arch="$2" expect_filepat="$3" expect_objarch="$4"
  local size
  size="$(file_bytes "$art")"
  [ "$size" -gt 0 ] || die "artifact is empty: $art"
  [ "$size" -le "$MAX_ARTIFACT_BYTES" ] || die "artifact $size bytes exceeds bound $MAX_ARTIFACT_BYTES"

  local ftype
  ftype="$(file -b "$art")" || die "file(1) failed on artifact"
  case "$ftype" in
    *"$expect_filepat"*) ;;
    *) die "not an ELF 64-bit $expect_arch shared object: $ftype" ;;
  esac
  printf 'build_opentui: file type ok: %s\n' "$ftype" >&2

  # Architecture ground truth via objdump -f (binutils works cross-format here).
  local archline
  archline="$(objdump -f "$art" 2>/dev/null | awk -F': ' '/^architecture:/{print $2; exit}')" \
    || die "objdump -f failed on artifact"
  [ "$archline" = "$expect_objarch" ] || die "objdump architecture '$archline' is not '$expect_objarch'"
  printf 'build_opentui: objdump architecture ok: %s\n' "$archline" >&2

  # Required exported ABI symbols via dynamic symbol table (undecorated names).
  local dynsym
  if dynsym="$(nm -D --defined-only "$art" 2>/dev/null)"; then
    :
  else
    dynsym="$(objdump -T "$art" 2>/dev/null | awk '$NF ~ /^[A-Za-z_][A-Za-z0-9_]*$/ {print $NF}')" \
      || die "nm -D and objdump -T both failed on artifact"
  fi
  local sym missing=0 nl padded
  nl="$(printf '\n_')"
  nl="${nl%_}"
  padded="$nl$dynsym$nl"
  for sym in $REQUIRED_SYMBOLS; do
    case "$padded" in
      *" ${sym}${nl}"*|*"	${sym}${nl}"*|*"${nl}${sym}${nl}"*) ;;
      *)
        printf 'build_opentui: error: missing exported symbol: %s\n' "$sym" >&2
        missing=1 ;;
    esac
  done
  [ "$missing" -eq 0 ] || die "required ABI symbols absent"
  printf 'build_opentui: ABI symbols ok: %s\n' "$REQUIRED_SYMBOLS" >&2

  # DT_NEEDED allowlist: bare sonames only, never absolute paths.
  local needed lib bad=0
  needed="$(objdump -p "$art" 2>/dev/null | awk '$1=="NEEDED"{print $2}')" \
    || die "objdump -p failed on artifact"
  [ -n "$needed" ] || die "no DT_NEEDED entries found"
  while IFS= read -r lib; do
    [ -n "$lib" ] || continue
    case "$lib" in
      */*|*" "*|*..*) die "unsafe NEEDED entry rejected: $lib" ;;
    esac
    case " $ELF_ALLOWED_NEEDED " in
      *" $lib "*) printf 'build_opentui: NEEDED ok: %s\n' "$lib" >&2 ;;
      *) die "unexpected DT_NEEDED library rejected: $lib" ;;
    esac
  done <<EOF_NEEDED
$needed
EOF_NEEDED
  [ "$bad" -eq 0 ] || die "unsafe NEEDED entries rejected"

  # RPATH/RUNPATH: absent, or $ORIGIN-relative only. Absolute leakage fails closed.
  local rpaths
  rpaths="$(objdump -p "$art" 2>/dev/null | awk '$1=="RPATH"||$1=="RUNPATH"{sub(/^[^ ]+ +/,""); print}')" \
    || die "objdump RPATH scan failed"
  if [ -n "$rpaths" ]; then
    while IFS= read -r rpath; do
      [ -n "$rpath" ] || continue
      case "$rpath" in
        '$ORIGIN'*|"\$ORIGIN"*)
          printf 'build_opentui: runpath ok (relative): %s\n' "$rpath" >&2 ;;
        /*|*..*)
          die "unsafe absolute runpath rejected: $rpath" ;;
        *) die "unrecognized runpath rejected: $rpath" ;;
      esac
    done <<EOF_RUNPATH
$rpaths
EOF_RUNPATH
  else
    printf 'build_opentui: no RPATH/RUNPATH entries (ok)\n' >&2
  fi

  sha256_of "$art"
}

# PE export-table ground truth comes from a bounded stdlib PE parser
# (pe_exports, static code, paths are argv only). pe_export_def writes a
# complete sorted .def (LIBRARY + EXPORTS + every export name) for dlltool.
pe_exports() {
  python3 - "$1" <<'PYEOF'
import struct, sys
path = sys.argv[1]
data = open(path, 'rb').read()
if len(data) < 64 or data[0:2] != b'MZ':
    sys.exit('not MZ')
(e_lfanew,) = struct.unpack_from('<I', data, 0x3C)
if data[e_lfanew:e_lfanew+4] != b'PE\0\0':
    sys.exit('not PE')
machine, nsec, _, _, _, optsize, chars = struct.unpack_from('<HHIIIHH', data, e_lfanew+4)
if machine != 0x8664:
    sys.exit('machine is not AMD64')
if not (chars & 0x2000):
    sys.exit('not DLL')
opt = e_lfanew + 24
(magic,) = struct.unpack_from('<H', data, opt)
if magic != 0x20B:
    sys.exit('not PE32+')
(rva, _) = struct.unpack_from('<II', data, opt+112)
if not rva:
    sys.exit('no export table')
secoff = e_lfanew + 24 + optsize
secs = []
for i in range(nsec):
    o = secoff + 40*i
    vs, vaddr, rawsz, rawptr = struct.unpack_from('<IIII', data, o+8)
    secs.append((vaddr, max(vs, rawsz), rawptr))
def rva2off(r):
    for vaddr, size, rawptr in secs:
        if vaddr <= r < vaddr + size:
            return rawptr + (r - vaddr)
    sys.exit('export RVA outside sections')
e = rva2off(rva)
(nnames,) = struct.unpack_from('<I', data, e+24)
(addr_names,) = struct.unpack_from('<I', data, e+32)
names = set()
for i in range(nnames):
    (nrva,) = struct.unpack_from('<I', data, rva2off(addr_names)+4*i)
    end = rva2off(nrva)
    end2 = data.index(b'\x00', end)
    name = data[end:end2].decode('ascii')
    case_ok = all(32 < ord(c) < 127 and c not in ' ",;=' for c in name)
    if not name or not case_ok:
        sys.exit('unsafe export name rejected')
    names.add(name)
for want in ('createRenderer', 'destroyRenderer', 'getCurrentBuffer', 'bufferDrawText', 'bufferWriteResolvedChars'):
    if want not in names:
        sys.exit('missing export: ' + want)
print('\n'.join(sorted(names)))
PYEOF
}

pe_export_def() {
  local dll="$1" def="$2" list
  list="$(pe_exports "$dll")" || return 1
  [ -n "$list" ] || die "empty DLL export table"
  {
    printf 'LIBRARY opentui.dll\nEXPORTS\n'
    printf '%s\n' "$list"
  } > "$def" || die "cannot write export .def"
  local def_bytes
  def_bytes="$(file_bytes "$def")"
  [ "$def_bytes" -le 1048576 ] || die "export .def exceeds 1 MiB bound"
}

# DLL-only pre-check before .def derivation (file type + bounds). Required
# exports are enforced by pe_export_def itself; full pair checks run after
# dlltool via verify_pe.
verify_pe_dll_only() {
  local dll="$1" size ftype
  size="$(file_bytes "$dll")"
  [ "$size" -gt 0 ] || die "DLL is empty: $dll"
  [ "$size" -le "$MAX_ARTIFACT_BYTES" ] || die "DLL $size bytes exceeds bound $MAX_ARTIFACT_BYTES"
  ftype="$(file -b "$dll")" || die "file(1) failed on DLL"
  case "$ftype" in
    *"PE32+ executable (DLL)"*"x86-64"*)
      printf 'build_opentui: file type ok: %s\n' "$ftype" >&2 ;;
    *) die "not a PE32+ x86-64 DLL: $ftype" ;;
  esac
}

# PE64 DLL + import-lib verifier. Import-lib/export correspondence is
# complete: every DLL export name occurs in the archive string table and
# every archive DLL-reference names opentui.dll.
verify_pe() {
  local dll="$1" implib="$2"
  local size
  size="$(file_bytes "$dll")"
  [ "$size" -gt 0 ] || die "DLL is empty: $dll"
  [ "$size" -le "$MAX_ARTIFACT_BYTES" ] || die "DLL $size bytes exceeds bound $MAX_ARTIFACT_BYTES"
  size="$(file_bytes "$implib")"
  [ "$size" -gt 0 ] || die "import lib is empty: $implib"
  [ "$size" -le "$MAX_ARTIFACT_BYTES" ] || die "import lib $size bytes exceeds bound $MAX_ARTIFACT_BYTES"

  local ftype
  ftype="$(file -b "$dll")" || die "file(1) failed on DLL"
  case "$ftype" in
    *"PE32+ executable (DLL)"*"x86-64"*|*"PE32+ executable (DLL) (console) x86-64"*)
      printf 'build_opentui: file type ok: %s\n' "$ftype" >&2 ;;
    *) die "not a PE32+ x86-64 DLL: $ftype" ;;
  esac

  # Import library must be a real archive (ar magic), matching one of the
  # build.rs-accepted names: opentui.lib (MSVC-style) or libopentui.dll.a (GNU).
  local base magic
  base="$(basename -- "$implib")"
  case "$base" in
    opentui.lib|libopentui.dll.a) ;;
    *) die "import lib name '$base' is not build.rs-accepted (opentui.lib or libopentui.dll.a)" ;;
  esac
  magic="$(head -c 8 "$implib")" || die "cannot read import lib magic"
  [ "$magic" = "!<arch>" ] || die "import lib is not an ar archive: $implib"
  printf 'build_opentui: import lib ok: %s\n' "$base" >&2

  command -v python3 >/dev/null 2>&1 || die "required tool missing for PE verify: python3"
  local exports export missing=0
  exports="$(pe_exports "$dll")" || die "PE export-table check failed on DLL"
  local nl padded
  nl="$(printf '\n_')"
  nl="${nl%_}"
  padded="$nl$exports$nl"
  for export in $REQUIRED_SYMBOLS; do
    case "$padded" in
      *"${nl}${export}${nl}"*) ;;
      *) printf 'build_opentui: error: missing export: %s\n' "$export" >&2; missing=1 ;;
    esac
  done
  [ "$missing" -eq 0 ] || die "required DLL exports absent"
  local count
  count="$(printf '%s\n' "$exports" | wc -l | tr -d '[:space:]')"
  printf 'build_opentui: PE exports ok: %s required of %s total\n' "5" "$count" >&2

  # Complete correspondence: every DLL export resolves inside the import
  # lib string table; the archive references exactly opentui.dll.
  command -v strings >/dev/null 2>&1 || die "required tool missing for PE verify: strings"
  local archive_strings
  archive_strings="$(strings -a "$implib" 2>/dev/null)" || die "strings failed on import lib"
  padded="$nl$archive_strings$nl"
  local name bad=0
  while IFS= read -r name; do
    [ -n "$name" ] || continue
    case "$padded" in
      *"${nl}${name}${nl}"*|*" ${name}${nl}"*|*"	${name}${nl}"*) ;;
      *) printf 'build_opentui: error: DLL export missing from import lib: %s\n' "$name" >&2; bad=1 ;;
    esac
  done <<EOF_EXPORTS
$exports
EOF_EXPORTS
  [ "$bad" -eq 0 ] || die "incomplete export/import correspondence"
  case "$padded" in
    *"opentui.dll"*) ;;
    *) die "import lib does not reference opentui.dll" ;;
  esac
  printf 'build_opentui: export/import correspondence ok: %s exports\n' "$count" >&2
}

# Dispatch verify by rust triple. Prints SHA(s) on stdout.
verify_for_target() {
  local triple="$1" art="$2" kind
  kind="$(target_kind "$triple")"
  case "$kind" in
    macho)
      case "$triple" in
        aarch64-apple-darwin) verify_macho "$art" "arm64" "arm64" ;;
        x86_64-apple-darwin) verify_macho "$art" "x86_64" "x86_64" ;;
      esac
      ;;
    elf)
      case "$triple" in
        x86_64-unknown-linux-gnu) verify_elf "$art" "x86_64" "x86-64" "x86_64" ;;
        aarch64-unknown-linux-gnu) verify_elf "$art" "aarch64" "aarch64" "aarch64" ;;
      esac
      ;;
    pe)
      die "--verify-only for $triple needs the DLL and its import lib: use --verify-only <dll> --import-lib <lib>" ;;
    unsupported-msvc)
      die "$triple is not producible by the pinned interface (SUPPORTED_TARGETS lists only *-windows-gnu); nothing to verify" ;;
    *) usage_die "unknown --target: $triple" ;;
  esac
}

# ---------------------------------------------------------------- main ------
MODE="build"
VERIFY_PATH=""
VERIFY_IMPORT_LIB=""
WORK_DIR=""
OUTPUT_DIR=""
RUST_TARGET="$DEFAULT_RUST_TARGET"
LINKAGE="dynamic"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --help|-h) print_help; exit 0 ;;
    --verify-only)
      [ "$#" -ge 2 ] || usage_die "--verify-only needs an artifact path"
      MODE="verify"; VERIFY_PATH="$2"; shift 2 ;;
    --import-lib)
      [ "$#" -ge 2 ] || usage_die "--import-lib needs a library path"
      VERIFY_IMPORT_LIB="$2"; shift 2 ;;
    --work-dir)
      [ "$#" -ge 2 ] || usage_die "--work-dir needs a directory"
      WORK_DIR="$2"; shift 2 ;;
    --output-dir)
      [ "$#" -ge 2 ] || usage_die "--output-dir needs a directory"
      OUTPUT_DIR="$2"; shift 2 ;;
    --target)
      [ "$#" -ge 2 ] || usage_die "--target needs a rust triple"
      RUST_TARGET="$2"; shift 2 ;;
    --static)
      LINKAGE="static"; shift ;;
    --) shift; break ;;
    -*) usage_die "unknown flag: $1" ;;
    *) usage_die "unexpected argument: $1" ;;
  esac
done

KIND="$(target_kind "$RUST_TARGET")"
[ "$KIND" = "unknown" ] && usage_die "unknown --target: $RUST_TARGET (see --help for the supported matrix)"
if [ "$KIND" = "unsupported-msvc" ]; then
  die "$RUST_TARGET is not producible: the pinned build.zig SUPPORTED_TARGETS lists only x86_64/aarch64-windows-gnu, no MSVC ABI entry; refusing to invent one"
fi
if [ "$LINKAGE" = "static" ] && [ "$RUST_TARGET" != "aarch64-apple-darwin" ]; then
  die "--static is currently verified only for aarch64-apple-darwin"
fi
ZIG_TARGET="$(target_zig "$RUST_TARGET")"
ZIG_OUTPUT_NAME="$(target_outname "$RUST_TARGET")"
EXPECTED_LIB="$(target_dll_name "$RUST_TARGET")"
[ -n "$ZIG_TARGET$ZIG_OUTPUT_NAME$EXPECTED_LIB" ] || die "internal target-table gap for $RUST_TARGET"

if [ "$MODE" = "verify" ]; then
  if [ -n "$WORK_DIR" ] || [ -n "$OUTPUT_DIR" ]; then
    usage_die "--verify-only takes no --work-dir/--output-dir"
  fi
  [ -z "$VERIFY_PATH" ] && usage_die "--verify-only needs an artifact path"
  if [ "$KIND" = "pe" ]; then
    [ -n "$VERIFY_IMPORT_LIB" ] || die "--verify-only for $RUST_TARGET needs --import-lib <import-lib> beside the DLL"
    verify_pe "$VERIFY_PATH" "$VERIFY_IMPORT_LIB"
    printf '%s  %s\n' "$(sha256_of "$VERIFY_PATH")" "$VERIFY_PATH"
    printf '%s  %s\n' "$(sha256_of "$VERIFY_IMPORT_LIB")" "$VERIFY_IMPORT_LIB"
    exit 0
  fi
  [ -n "$VERIFY_IMPORT_LIB" ] && usage_die "--import-lib only applies to the x86_64-pc-windows-gnu target"
  if [ "$LINKAGE" = "static" ]; then
    sum="$(verify_static_macho "$VERIFY_PATH")"
  else
    sum="$(verify_for_target "$RUST_TARGET" "$VERIFY_PATH")"
  fi
  printf '%s  %s\n' "$sum" "$VERIFY_PATH"
  exit 0
fi

[ -n "$VERIFY_IMPORT_LIB" ] && usage_die "--import-lib only applies with --verify-only for x86_64-pc-windows-gnu"
[ -n "$WORK_DIR" ] || usage_die "--work-dir is required"
[ -n "$OUTPUT_DIR" ] || usage_die "--output-dir is required"
require_abs_dir_outside_repo "--work-dir" "$WORK_DIR"
require_abs_dir_outside_repo "--output-dir" "$OUTPUT_DIR"

for tool in curl git tar shasum file nm objdump strings sh cksum awk sed ar; do
  command -v "$tool" >/dev/null 2>&1 || die "required tool missing: $tool"
done
if [ "$KIND" = "macho" ]; then
  for tool in otool lipo; do
    command -v "$tool" >/dev/null 2>&1 || die "required tool missing for macOS target: $tool"
  done
fi
if [ "$LINKAGE" = "static" ]; then
  for tool in libtool ranlib strip; do
    command -v "$tool" >/dev/null 2>&1 || die "required tool missing for static macOS target: $tool"
  done
fi
if [ "$KIND" = "pe" ]; then
  command -v python3 >/dev/null 2>&1 || die "required tool missing for Windows target: python3"
fi
[ "$(uname -s)" = "Darwin" ] || die "this builder supports a Darwin/arm64 host only (sole pinned Zig archive is zig-aarch64-macos)"
[ "$(uname -m)" = "arm64" ] || die "this builder supports a Darwin/arm64 host only (sole pinned Zig archive is zig-aarch64-macos)"

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

if [ "$LINKAGE" = "static" ]; then
  STATIC_PATCH="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)/linkage-static.patch"
  [ -f "$STATIC_PATCH" ] || die "pinned static linkage patch missing"
  [ "$(sha256_of "$STATIC_PATCH")" = "$STATIC_PATCH_SHA256" ] || die "static linkage patch SHA-256 mismatch"
  BUILD_ZIG="$SRC_DIR/$ZIG_BUILD_SUBDIR/build.zig"
  [ "$(sha256_of "$BUILD_ZIG")" = "$STATIC_BUILD_ZIG_PREIMAGE_SHA256" ] || die "static build.zig preimage mismatch"
  git -C "$SRC_DIR" apply --check "$STATIC_PATCH" || die "static linkage patch does not apply cleanly"
  git -C "$SRC_DIR" apply "$STATIC_PATCH" || die "static linkage patch apply failed"
  [ "$(sha256_of "$BUILD_ZIG")" = "$STATIC_BUILD_ZIG_POSTIMAGE_SHA256" ] || die "static build.zig postimage mismatch"
  printf 'build_opentui: static linkage patch hashes ok\n' >&2
fi

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
# macOS SDK is mandatory for *-macos targets (CoreAudio/AppKit); other
# targets cross-compile from this host with no SDK flag.
MACOS_SDK=""
if [ "$KIND" = "macho" ]; then
  for candidate in "${SDKROOT:-}" "${MACOS_SDK_PATH:-}" "${MACOSX_SDK_PATH:-}" \
      /Library/Developer/CommandLineTools/SDKs/MacOSX.sdk; do
    if [ -n "$candidate" ] && [ -d "$candidate/System/Library/Frameworks/CoreFoundation.framework" ]; then
      MACOS_SDK="$candidate"
      break
    fi
  done
  [ -n "$MACOS_SDK" ] || die "macOS SDK with CoreFoundation.framework not found; set SDKROOT"
fi

printf 'build_opentui: building %s for %s (%s)\n' "$ZIG_TARGET" "$RUST_TARGET" "$OPTIMIZE_MODE" >&2
if [ "$KIND" = "macho" ]; then
  zig_build_args=("$ZIG_BIN" build "-Dlibrary-target=$ZIG_TARGET" "-Doptimize=$OPTIMIZE_MODE" "-Dmacos-sdk=$MACOS_SDK")
else
  zig_build_args=("$ZIG_BIN" build "-Dlibrary-target=$ZIG_TARGET" "-Doptimize=$OPTIMIZE_MODE")
fi
[ "$LINKAGE" = "static" ] && zig_build_args+=("-Dlinkage=static")
(cd -- "$SRC_DIR/$ZIG_BUILD_SUBDIR" && "${zig_build_args[@]}") || die "zig build failed"

BUILT_DIR="$SRC_DIR/$ZIG_BUILD_SUBDIR/lib/$ZIG_OUTPUT_NAME"
[ -d "$BUILT_DIR" ] || die "expected build output dir missing: $BUILT_DIR (interface drift?)"

# ------------------------------------------------------------ 5. verify ----
if [ "$KIND" = "pe" ]; then
  # Windows: DLL plus a complete import lib derived deterministically from
  # the verified DLL export table with the pinned zig dlltool (llvm-dlltool
  # -m i386:x86-64, same backend that emits zig-out/lib/opentui.lib; probed
  # byte-identical for the same input). zig build installs only DLL+PDB into
  # lib/<out>, so the build.rs-accepted libopentui.dll.a is generated here
  # from ALL DLL exports, never from a 5-symbol subset. Never publish a lone DLL.
  printf 'build_opentui: build output listing for %s:\n' "$ZIG_OUTPUT_NAME" >&2
  ls -la -- "$BUILT_DIR" >&2 || die "cannot list build output dir"
  BUILT_DLL="$BUILT_DIR/$EXPECTED_LIB"
  [ -f "$BUILT_DLL" ] || die "expected DLL missing: $BUILT_DLL"
  printf 'build_opentui: verifying built DLL exports\n' >&2
  verify_pe_dll_only "$BUILT_DLL"
  EXPORT_DEF="$TMP_ROOT/opentui.def"
  pe_export_def "$BUILT_DLL" "$EXPORT_DEF"
  export_count="$(wc -l < "$EXPORT_DEF" | tr -d '[:space:]')"
  export_count="$((export_count - 2))"
  printf 'build_opentui: export .def ok: %s exports\n' "$export_count" >&2
  BUILT_IMPORT="$TMP_ROOT/libopentui.dll.a"
  dlltool_args=("$ZIG_BIN" dlltool -D opentui.dll -d "$EXPORT_DEF" -l "$BUILT_IMPORT" -m i386:x86-64)
  "${dlltool_args[@]}" || die "pinned zig dlltool import-lib generation failed"
  [ -f "$BUILT_IMPORT" ] || die "dlltool produced no import lib"
  printf 'build_opentui: verifying built Windows pair\n' >&2
  verify_pe "$BUILT_DLL" "$BUILT_IMPORT"
  built_sum="$(sha256_of "$BUILT_DLL")"
  built_import_sum="$(sha256_of "$BUILT_IMPORT")"
  printf 'build_opentui: built DLL sha: %s\n' "$built_sum" >&2
  printf 'build_opentui: built import-lib sha: %s\n' "$built_import_sum" >&2

  # ------------------------------------------------------------ 6. copy ----
  # Atomic publish of the complete pair: stage both, then rename both.
  # The GNU import-lib filename is the build.rs-accepted libopentui.dll.a.
  IMPORT_BASE="libopentui.dll.a"
  STAGED_DLL="$OUTPUT_DIR/.$EXPECTED_LIB.tmp.$$"
  STAGED_IMPORT="$OUTPUT_DIR/.$IMPORT_BASE.tmp.$$"
  cp_args=(cp -- "$BUILT_DLL" "$STAGED_DLL")
  "${cp_args[@]}" || die "stage copy of DLL failed"
  cp_import_args=(cp -- "$BUILT_IMPORT" "$STAGED_IMPORT")
  "${cp_import_args[@]}" || die "stage copy of import lib failed"
  mv_args=(mv -f -- "$STAGED_DLL" "$OUTPUT_DIR/$EXPECTED_LIB")
  "${mv_args[@]}" || die "atomic publish of DLL failed"
  mv_import_args=(mv -f -- "$STAGED_IMPORT" "$OUTPUT_DIR/$IMPORT_BASE")
  "${mv_import_args[@]}" || die "atomic publish of import lib failed"
  trap - EXIT HUP INT TERM
  cleanup

  final_sum="$(sha256_of "$OUTPUT_DIR/$EXPECTED_LIB")"
  [ "$final_sum" = "$built_sum" ] || die "published DLL hash changed during copy"
  final_import_sum="$(sha256_of "$OUTPUT_DIR/$IMPORT_BASE")"
  [ "$final_import_sum" = "$built_import_sum" ] || die "published import lib hash changed during copy"
  printf '%s  %s\n' "$final_sum" "$OUTPUT_DIR/$EXPECTED_LIB"
  printf '%s  %s\n' "$final_import_sum" "$OUTPUT_DIR/$IMPORT_BASE"
  exit 0
fi

if [ "$LINKAGE" = "static" ]; then
  BUILT_STATIC="$BUILT_DIR/libopentui.a"
  [ -f "$BUILT_STATIC" ] || die "expected static build output missing: $BUILT_STATIC"
  REPACK_DIR="$TMP_ROOT/static-members"
  mkdir -p -- "$REPACK_DIR" || die "cannot create static repack dir"
  (cd -- "$REPACK_DIR" && ar -x "$BUILT_STATIC") || die "cannot extract static build output"
  rm -f -- "$REPACK_DIR/__.SYMDEF" "$REPACK_DIR/__.SYMDEF SORTED"
  static_objects=("$REPACK_DIR"/*.o)
  [ -f "${static_objects[0]}" ] || die "static build output has no object members"
  chmod u+rw "${static_objects[@]}" || die "cannot normalize static object permissions"
  # ReleaseSafe object files retain DWARF paths rooted in the caller's random
  # build directory. Remove only debug symbols before repacking so identical
  # pinned inputs produce identical distributable archives across clean roots.
  for static_object in "${static_objects[@]}"; do
    strip -S "$static_object" || die "cannot strip nondeterministic debug data from static object"
  done
  REPACKED_STATIC="$TMP_ROOT/$EXPECTED_LIB"
  ZERO_AR_DATE=1 libtool -static -o "$REPACKED_STATIC" "${static_objects[@]}" || die "Apple-compatible static repack failed"
  ZERO_AR_DATE=1 ranlib "$REPACKED_STATIC" || die "ranlib failed on static artifact"
  built_sum="$(verify_static_macho "$REPACKED_STATIC")"
  verify_static_exports_from_source "$REPACKED_STATIC" "$SRC_DIR/$ZIG_BUILD_SUBDIR/src/lib.zig"

  STAGED="$OUTPUT_DIR/.$EXPECTED_LIB.tmp.$$"
  cp -- "$REPACKED_STATIC" "$STAGED" || die "stage copy of static artifact failed"
  mv -f -- "$STAGED" "$OUTPUT_DIR/$EXPECTED_LIB" || die "atomic publish of static artifact failed"
  trap - EXIT HUP INT TERM
  cleanup
  final_sum="$(sha256_of "$OUTPUT_DIR/$EXPECTED_LIB")"
  [ "$final_sum" = "$built_sum" ] || die "published static artifact hash changed during copy"
  printf '%s  %s\n' "$final_sum" "$OUTPUT_DIR/$EXPECTED_LIB"
  exit 0
fi

BUILT_LIB="$BUILT_DIR/$EXPECTED_LIB"
[ -f "$BUILT_LIB" ] || die "expected build output missing: $BUILT_LIB"

printf 'build_opentui: verifying built artifact\n' >&2
built_sum="$(verify_for_target "$RUST_TARGET" "$BUILT_LIB")"
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
