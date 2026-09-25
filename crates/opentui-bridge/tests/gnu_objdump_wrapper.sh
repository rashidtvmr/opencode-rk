#!/usr/bin/env bash
# TUI-011 GNU objdump architecture regression (independent RED test author).
#
# Contract (frozen by the controller, never edited by the implementer):
#   `build_opentui.sh --verify-only <x86_64 ELF> --target x86_64-unknown-linux-gnu`
#   must accept the GNU binutils objdump architecture line
#       architecture: i386:x86-64, flags 0x00000150:
#   as the x86_64 artifact architecture. The current parser compares the whole
#   field against the literal `x86_64` (which only LLVM/Apple objdump emits) and
#   therefore fails closed on every real GNU/Linux host that produced this very
#   artifact.
#
# This test is deterministic and host-independent: it forces GNU `objdump -f`
# output through a PATH-prepended wrapper while delegating every other objdump
# operation to the real objdump. No network, no repository writes, no user data,
# no secrets, bounded tempdir removed on exit.
#
# Expected states:
#   RED  (current parser): nonzero exit, stderr contains
#     "objdump architecture 'i386:x86-64, flags 0x00000150:' is not 'x86_64'".
#   GREEN (fixed parser): exit 0 and stdout carries the artifact SHA-256 line.
#
# Run: bash crates/opentui-bridge/tests/gnu_objdump_wrapper.sh

set -euo pipefail

ROOT="$(cd -- "$(dirname -- "$0")/../../.." && pwd -P)"
SCRIPT="$ROOT/crates/opentui-bridge/native/build_opentui.sh"
ARTIFACT="$ROOT/crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu/libopentui.so"
TARGET="x86_64-unknown-linux-gnu"

# Pinned identity of the real tracked x86_64 Linux artifact.
readonly EXPECT_SHA256="9f074adf1e3c67bb027433d44da05b9e285a37ae58cf0b2ed76504304450c79e"
readonly EXPECT_SIZE="26627832"
readonly GNU_ARCH_LINE="architecture: i386:x86-64, flags 0x00000150:"

fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }
note() { printf '%s\n' "$*"; }

[ -f "$SCRIPT" ] || fail "missing builder: $SCRIPT"
[ -f "$ARTIFACT" ] || fail "missing x86_64 Linux artifact: $ARTIFACT"

# The real objdump must be resolved before the wrapper shadows it in PATH.
REAL_OBJDUMP="$(command -v objdump || true)"
[ -n "$REAL_OBJDUMP" ] || fail "objdump is required to delegate non -f operations"

# sha256sum on GNU/Linux, shasum on macOS: same digest, host-independent.
if command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum "$1" | awk '{print $1}'; }
else
  sha256() { shasum -a 256 "$1" | awk '{print $1}'; }
fi

# Verify the artifact is the real tracked one (fail closed on drift).
SHA="$(sha256 "$ARTIFACT")"
SIZE="$(wc -c < "$ARTIFACT" | tr -d '[:space:]')"
[ "$SHA" = "$EXPECT_SHA256" ] || fail "artifact SHA-256 mismatch: $SHA != $EXPECT_SHA256"
[ "$SIZE" = "$EXPECT_SIZE" ] || fail "artifact size mismatch: $SIZE != $EXPECT_SIZE"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/tui011-gnu-objdump.XXXXXX")" || fail "mktemp failed"
cleanup() { rm -rf -- "$TMP"; }
trap cleanup EXIT HUP INT TERM

WRAPPER="$TMP/objdump"
cat > "$WRAPPER" <<EOF_WRAPPER
#!/bin/sh
if [ "\$1" = "-f" ]; then
  printf '%s\n' '$GNU_ARCH_LINE'
  printf 'start address: 0x0000000000000000\n'
  exit 0
fi
exec "$REAL_OBJDUMP" "\$@"
EOF_WRAPPER
chmod 0755 "$WRAPPER" || fail "cannot chmod wrapper"

# Sanity: the wrapper must emit the forced GNU line and delegate -p/-T intact.
[ "$("$WRAPPER" -f "$ARTIFACT")" = "$GNU_ARCH_LINE
start address: 0x0000000000000000" ] || fail "wrapper -f output is not the forced GNU line"
"$WRAPPER" -p "$ARTIFACT" > "$TMP/delegated.p" 2>/dev/null \
  || fail "wrapper did not delegate objdump -p to the real objdump"

note "Running builder with GNU objdump wrapper forced first in PATH"

set +e
PATH="$TMP:$PATH" bash "$SCRIPT" --verify-only "$ARTIFACT" --target "$TARGET" \
  > "$TMP/out" 2> "$TMP/err"
RC=$?
set -e

note "--- builder exit: $RC"
note "--- builder stdout"
cat "$TMP/out"
note "--- builder stderr"
cat "$TMP/err"

# RED must fail specifically on the GNU architecture line.
if [ "$RC" -ne 0 ]; then
  if grep -qF "objdump architecture 'i386:x86-64, flags 0x00000150:' is not 'x86_64'" "$TMP/err"; then
    fail "RED: builder rejects the GNU objdump architecture line (current parser bug)"
  fi
  fail "builder failed for an unexpected reason (exit $RC)"
fi

# GREEN must accept the GNU line and print the artifact SHA-256 on stdout.
grep -qF "objdump architecture ok: i386:x86-64" "$TMP/err" \
  || fail "GREEN: builder did not report the GNU architecture as accepted"
grep -qF "$EXPECT_SHA256" "$TMP/out" \
  || fail "GREEN: stdout did not carry the artifact SHA-256 line"

note "PASS: GNU objdump architecture i386:x86-64 accepted; artifact sha $EXPECT_SHA256"
