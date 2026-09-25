# PACKAGE-POSIX-INSTALLER-VERIFY

## Scope

- Task: `PACKAGE-POSIX-INSTALLER-VERIFY`
- Branch: `verify/PACKAGE-POSIX-INSTALLER`
- Candidate: `2f87242d6371cc5059facb75ff6756b0ece6ed68`
- Owned path: this scratchpad plus this task's ledger row. No product/script/test edits.
- Consumer verdict: ACCEPT-SCOPED for POSIX installer consumer. Not producer, release, or authenticity acceptance.

## Frozen artifact

- `crates/cli/tests/packaged_revision_binding.rs` SHA-256:
  `d0a800ed7bda8b623aa427604c6ad2c28acdd37633d391079d651246ad0a8858`
- Hash unchanged before/after verification.

## Required commands

- `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-cli --test packaged_revision_binding -- --test-threads=1`
  - PASS: `5 passed (1 suite, 2.29s)`.
  - Initial Python timeout wrapper attempt was invalid on this macOS host (`rtk` interpreted assignment as a command); rerun used the exact cargo command with caps and passed.
- `sh -n scripts/install-oc2.sh scripts/install-opencode2.sh scripts/install-oc2.ps1 scripts/install-opencode2.ps1`
  - PASS, no output. (`sh -n` on PS1 is syntax-only shell invocation; actual POSIX scripts separately checked.)
- `dash -n scripts/install-oc2.sh && dash -n scripts/install-opencode2.sh`
  - PASS, no output.
- `git diff --check`
  - PASS, no output.

## Independent disposable probes

Generated temporary tar/gzip fixtures under `/var/folders/.../T/oc2-probe-*`; all removed. No user data touched; no survivors.

- Positive canonical archive/runtime receipt: PASS, exit `0`; observed
  `oc2 0.1.0-alpha.1 revision=0123456789abcdef0123456789abcdef01234567`.
- Environment scrub: PASS in the positive fixture. Runtime payload attempted to observe `SECRET_SENTINEL` and write a marker; installer runtime execution uses `env -i PATH=... LC_ALL=C` at `scripts/install-oc2.sh:423-431`, marker absent.
- Legacy collision pre-mutation: PASS, exit `73`; existing `install/bin/opencode` preserved; old `oc2` and native sentinels preserved.
- Canonical USTAR checksum rejection: PASS, exit `65`; old sentinels preserved.
- Per-member bound rejection: PASS, exit `65` for declared `134217729`-byte native member; old sentinels preserved.
- Post-install receipt mismatch rollback: PASS, exit `74`; old binary/native bytes restored; transaction temporary files absent.
- Runtime timeout/process cleanup: PASS, exit `74`; timeout child marker absent after wait; recorded child PID no longer alive; old sentinels preserved.
- Temporary fixture root removed after probes.

## Source inspection

- Closed manifest grammar: `scripts/install-oc2.sh:153-227` accepts fixed ASCII canonical byte sequence, exact key order/types, fixed host member names, lowercase 64/40-hex values, final-byte equality, and `MAX_MANIFEST_BYTES=65536` at `20-25`, `707-710`.
- Archive/member bounds: compressed archive cap `20`, bounded gzip expansion `230-247`, 128 MiB member/aggregate caps `770-787`, SBOM 64 KiB cap `24`, exact three-member layout and EOF blocks `374-401`.
- Canonical USTAR: `306-371` verifies name, mode, uid/gid, size/mtime, typeflag, magic/version, zeroed fields, checksum; `374-401` verifies exact offsets, two zero EOF blocks, and no trailing bytes.
- Environment scrub: global locale/PATH normalization `6-10`; runtime `env -i` plus `PATH`/`LC_ALL` only `412-431`; known revision env fallbacks unset `414`.
- Timeout/process cleanup: owned PID/watchdog lifecycle `403-490`; timeout watchdog TERM/KILL group then PID fallback and parent wait `437-457`; cleanup repeats owned process teardown `492-506`; trap wiring `557-559`.
- Pre-mutation checks: regular non-symlink input and destination checks `681-701`, caps/hash/manifest/archive validation `703-765`, archive validation/extraction/runtime staging `767-845`, immediate destination recheck `847-860`, temp staging `862-871` before transaction activation `873`.
- Rollback: backups and installed-state flags `33-47`, rollback restores/removes both targets `522-546`; signal rollback `548-555`; transaction remains active through installed receipt observation `897-901`; backups removed only after success `903-906`.
- Post-install receipt: staged observation `841-845`, installed observation `897-901`; exact package/revision grammar and one-LF output `472-490`; receipt emitted only after transaction success `906`.

## Limit

POSIX consumer only. No producer, signing/authenticity, Windows runtime execution, or release acceptance claimed. Repository-wide convergence remains outside this verifier scope.
