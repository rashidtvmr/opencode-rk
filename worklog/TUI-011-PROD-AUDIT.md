# TUI-011 PROD AUDIT — Minimal fork build and reproducible native distribution

Status: blocked (audit-only handoff). No Cargo/product/test edits. No external dylib copy.
Task: TUI-011 (tasks/completion/tui.json). Deps: TUI-001.
Claim session: ses_f33364810ffe1Tn515VjEblNAr (reclaimed by ses_f3c4de578ffelQv59xDXmOs03B, re-claimed fresh).
Revision at start: 9ded2b901b2376a4a5ee1815a41c7a1234ece157 (branch lane/TUI-011-prod == origin main 9ded2b9).
Scratchpad owner file: worklog/TUI-011-PROD-AUDIT.md (only owned file this lane).

## 1. Source evidence (exact path/line)

- Pinned OpenTUI fork: rashidtvmr/opentui @ c01292fd0837bafd07ce458c74416b2b375a41ab
  - docs/audits/2026-09-17-app-completion.md:65-69 (OpenTUI decision section)
  - docs/architecture/COMPLETION_NATIVE_TUI.md:14-20 (Decision + boundaries)
  - docs/architecture/COMPLETION_NATIVE_TUI.md:16-17 (native tree = packages/native, build.zig defines native build)
  - crates/opentui-bridge/src/renderer.rs:3 (upstream: packages/native/src/lib.zig NativeHandle=u32)
  - crates/opentui-bridge/src/buffer.rs:7-15 (entry points: bufferDrawText:1800, bufferWriteResolvedChars:1791, etc.)
  - crates/opentui-bridge/src/text.rs:5-7 (branch rust-bridge)
  - crates/opentui-bridge/src/safe_renderer.rs:4-5 (FFI mirrors packages/native/src/lib.zig)
  - crates/opentui-bridge/src/safe_renderer.rs:88-94 (createRenderer sig matches lib.zig:1154-1208; null feed valid; mem dest=1)
  - crates/opentui-bridge/src/safe_renderer.rs:417-438 (draw_text null-fg SIGSEGV: pre-fix passed null fg; ptrToRGBA:107-109 derefs; bg nullable per optionalPtrToRGBA:111-117)
  - worklog/TUI-001-ABI-FIX.md:23-31 (fg fix: pack_rgba8(255,255,255,255,pack_meta(INTENT_DEFAULT,0)) per RGBA.ts:5,125-129)
  - worklog/TUI-001-ABI-REVERIFY.md (ABI re-verification: nm -gU exports _createRenderer, _bufferDrawText, _bufferWriteResolvedChars, _destroyRenderer; frozen test body SHA bc60682701678c08acc025e5238f68af923e6d0d1e38da010842efcdc99dc8e8)
  - worklog/LANE-TUI-LAND.md:138-141 (rebuild instructions in native/opentui-fork/PIN; orchestrator concern, not lane scope)

- Native artifact gating / link (no build.zig checked in; build is external):
  - crates/opentui-bridge/Cargo.toml:10 ([features] native=[] ; optional dep)
  - crates/opentui-bridge/Cargo.toml:16 (opencode-rk-opentui-bridge dev-dep with native feature)
  - crates/opentui-bridge/build.rs:2-6 (gates on CARGO_FEATURE_NATIVE; per-triple native/lib/<triple>)
  - crates/opentui-bridge/build.rs:9-10 (search path + rerun-if-changed)
  - crates/opentui-bridge/build.rs:11-55 (static_unix/linux_shared/mac_shared/windows variants; panic if missing:56-59)
  - crates/opentui-bridge/build.rs:52-55 (expected artifact names per OS: macos libopentui.a/.dylib, linux libopentui.a/.so, windows opentui.lib+opentui.dll / libopentui.dll.a+opentui.dll)
  - crates/opentui-bridge/src/safe_renderer.rs:157-160 (extern "C" { #[link(name="opentui")] ... })
  - crates/opentui-bridge/src/safe_renderer.rs:9 (no #![forbid(unsafe_code)] in crate — unsafe allowed only here)
  - crates/opentui-bridge/src/safe_renderer.rs:568 (render_once: real Rust caller via tui_entry::print_native_or_legacy)
  - crates/opentui-bridge/src/safe_renderer.rs:670-706 (frozen tests: zero/oversize/size rejected; render_once_content_present under [cfg(feature=native)])
  - crates/cli/src/tui_entry.rs:25-26 (cfg(feature=native) use opentui_bridge Renderer)
  - crates/cli/src/tui_entry.rs:943-956 (print_native_or_legacy: real render_once caller, 80x24 snapshot)
  - crates/cli/src/tui_entry.rs:947 (Renderer::render_once call)
  - crates/cli/Cargo.toml:39-40 (cli [features] native=["opencode-rk-opentui-bridge/native"])
  - crates/cli/Cargo.toml:42 (dev-dep opentui-bridge with native feature)

- No-subcommand native dispatch:
  - crates/cli/src/main.rs:53-71 (Cli: --native global flag; --once top-level alias)
  - crates/cli/src/main.rs:218-268 (None arm: plan_default_launch + discover_presence + creds_configured + LaunchMode::NativeTui -> if cli.native||cfg!(feature=native) dispatch to tui_entry::run_with_dir)
  - crates/cli/src/main.rs:241 (gate: cli.native || cfg!(feature = "native"))
  - crates/cli/src/main.rs:237-258 (NativeTui arm: prepare_daemon + TuiArgs + tui_entry::run_with_dir)

- Existing native_* pure-state modules (all #![forbid(unsafe_code)]):
  - crates/cli/src/native_shell.rs:1 (forbid unsafe; ShellBuffer bounded lines)
  - crates/cli/src/native_host.rs, native_layout.rs, native_composer.rs, native_palette.rs, native_navigation.rs, native_approvals.rs, native_status.rs, native_theme.rs, native_timeline.rs, native_transcript.rs
  - crates/cli/src/main.rs:37-48 (mod declarations)

- Install/loader/installer:
  - scripts/install-oc2.sh (checksum gate exit 65 mismatch; unsupported exit 64; archive-missing exit 66; platform exit 64; legacy opencode refusal exit 73; identity gate exit 74)
  - scripts/install-oc2.sh:1-3 (install oc2 into dir; fail-closed; never touches existing opencode binary)
  - scripts/install-oc2.sh:63-81 (sha256 gate; never overwrites legacy opencode exit 73)
  - scripts/install-oc2.sh:83-99 (tar extract; cp oc2; --version identity gate exit 74)
  - scripts/install-oc2.sh has NO native library bundling: libopentui.so/.dylib/.dll is NOT included in archive or extracted during install. Installer installs only the oc2 binary.
  - crates/cli/src/install_commands.rs:56-105 (InstallPlatform enum linux/mac/5 triples; install_platforms; release_artifact_ok gate; packaged_output_names_oc2)
  - crates/cli/src/install_commands.rs:44-53 (BINARY_NAME oc2, LEGACY opencode-rk, UNKNOWN_COMMAND_EXIT 64)
  - crates/cli/tests/packaging_identity.rs:1 (frozen FIX-PACKAGING tests, 5 scenarios, references SCRIPT_PATH /home/rashid/.../install-oc2.sh)
  - crates/cli/tests/packaging_identity.rs:268 (artifact path bug: ../native/lib/x86_64-linux/libopentui.so does not match actual x86_64-unknown-linux-gnu dir)

## 2. Exact revision / provenance pins

- OpenTUI fork commit: c01292fd0837bafd07ce458c74416b2b375a41ab (rashidtvmr/opentui)
- OpenTUI fork tree is packages/native (not packages/core/src/zig)
- OpenTUI fork native build defined in packages/native/build.zig (NOT checked into this repo)
- Native exports verified per nm: createRenderer, destroyRenderer, render, setupTerminal, restoreTerminalModes, getCurrentBuffer, bufferDrawText, bufferWriteResolvedChars, enableMouse, disableMouse, enableKittyKeyboard, disableKittyKeyboard, clearTerminal, resizeRenderer, setTerminalTitle, bufferFillRect, suspendRenderer, resumeRenderer
- OpenCode V2 baseline pin: 95daf90670b7c039c436c85537da5fbfe2205b41 (sources/upstream.lock.json)
- 9router pin: 17c4cc76877bd1755030a8414f8d0083f48dcccf (sources/upstream.lock.json)
- Repo HEAD at start: 9ded2b901b2376a4a5ee1815a41c7a1234ece157

## 3. Architecture / artifact matrix (current native state)

| Target triple | Vendored artifact | Location | Status |
|---|---|---|---|
| x86_64-unknown-linux-gnu | libopentui.so (25.4M, ELF, BuildID 2a63d3db5b86852e8284a0bb9c439801e2d246ea, debug_info, not stripped) | crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu/libopentui.so | PRESENT, committed-vendored |
| aarch64-apple-darwin | libopentui.dylib | (temp evidence-only path below) | ABSENT from repo |
| x86_64-apple-darwin | libopentui.a/libopentui.dylib | | ABSENT |
| x86_64-pc-windows-msvc | opentui.dll + opentui.lib | | ABSENT |
| aarch64-pc-windows-msvc | opentui.dll + opentui.lib | | ABSENT |

External genuine dylib (evidence-only, NOT copied, NOT vendored):
- Path: /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/opentui-pinned/packages/native/lib/aarch64-macos/libopentui.dylib
- Type: Mach-O 64-bit dynamically linked shared library arm64
- SHA-256: de3564d496a92fe48fd6ca24a975b1b886cbb654ad5b7eea15bfe81afbf80b50
- Used by TUI-001 ABI re-verify runs (DYLD_LIBRARY_PATH points here). Disposable temp fixture.
- nm -gU exports: _createRenderer, _getCurrentBuffer, _bufferDrawText, _bufferWriteResolvedChars, _destroyRenderer

build.rs (crates/opentui-bridge/build.rs) link gate:
- macos: libopentui.a (static) or libopentui.dylib (dylib)
- linux: libopentui.a (static) or libopentui.so (dylib)
- windows msvc: opentui.lib + opentui.dll; windows gnu: libopentui.dll.a + opentui.dll
- Missing artifact -> panic (build fails) under --features native

## 4. Loader/install behavior (current)

- scripts/install-oc2.sh: installs ONLY the oc2 binary into OC2_INSTALL_DIR (default $HOME/.local/bin).
  - Does NOT extract/bundled/libopentui.so or any native dylib into install dir.
  - Does NOT set rpath/runpath, install_name, or bundle side-by-side libopentui.
  - Does NOT create lib/ or native/lib under install dir.
  - No --native-lib flag; no LD_LIBRARY_PATH/DYLD guidance in installer output.
- build.rs emits cargo:rustc-link-search=native=.../native/lib/<triple> and cargo:rustc-link-lib=dylib|static=opentui.
  - Static link (libopentui.a) would bake renderer into binary; shared (libopentui.so/dylib/dll) requires runtime loader resolution (rpath on Unix, same-dir DLL on Windows).
- On Linux: libopentui.so is vendored and committed; dynamic link would need rpath or ldconfig or same-dir placement. Current binary does NOT set an rpath to native/lib (no bundle step).
- On macOS: NO libopentui.dylib vendored. dylib exists only in disposable temp. install_name would need @rpath or @executable_path/../lib. No rpath baked in by build.rs.
- On Windows: NO opentui.dll vendored. DLL must sit beside oc2.exe (loader searches exe dir). No bundling step.

## 5. SBOM / licenses / checksum

- License: MIT (repo Cargo.toml:license; sources/upstream.lock.json). OpenTUI fork MIT.
- NO SBOM artifact (no sbom.json, no LICENSE.third-party, no cargo-about/tern output in repo).
- NO per-platform checksum manifest for release artifacts (install-oc2.sh takes --checksum as CLI arg; caller supplies; no embedded manifest).
- NO checksums recorded for vendored libopentui.so in repo (no CHECKSUMS file, no .sha256). SHA-256 of the .so not pinned in source.
- License notices for transitive native deps (e.g., Zig runtime, harfbuzz, freetype if bundled in .so) NOT enumerated. libopentui.so is not stripped and is 25.4M, implying bundled deps; closure not audited in-repo.

## 6. Clean-target installed oc2 proof (test boundaries)

- crates/cli/tests/installed_default_entrypoint.rs: frozen RED test file (CONVERGENCE steps 1-4,10). 5 scenarios:
  - rerun_starts_from_empty_disposable_home (GREEN, line 84-93)
  - no_subcommand_launches_plan_path (RED, line 96-99; todo! body)
  - single_authenticated_daemon_without_manual_serve (RED, line 103-106; todo! body)
  - missing_credentials_open_inapp_setup (RED, line 109-112; todo! body)
  - native_render_once_snapshot_nonempty (RED, line 115-119; todo! body)
  - frozen_rerun_pinned_to_integrated_commit (RED, line 122-125; todo! body)
  - Script path: CARGO_BIN_EXE_opencode-rk (dev alias); packaged ships oc2.
- crates/cli/tests/native_launch.rs: Scenario A (default launch line-mode contract, line 165-252) + Scenario B (--native --once, line 261-321). BUG: line 268 path ../native/lib/x86_64-linux/libopentui.so does NOT match real dir crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu/. On macOS arm64 host, artifact_present=false -> expects non-zero exit + actionable msg (but TTY guard at tui_entry.rs:892-896 fails first: "refusing interactive TUI on piped stdin"). Pre-existing, not edited here.
- crates/cli/tests/packaging_identity.rs: frozen FIX-PACKAGING tests 5 scenarios, script at absolute path /home/rashid/... (non-portable). References --aliases/--download/--OC2_RELEASE_BASE (not yet in install-oc2.sh). RED per design.
- crates/cli/src/install_commands.rs:226-249 (frozen pure tests: no_subcommand_opens_native_tui, inventory matches upstream, oc2 identity, checksum fail-closed, unknown subcommand help).

## 7. Locally solvable vs human/external authority

### Locally solvable (repo-authority, implementation lanes):
1. Add rpath/bundle step in build.rs or a packaging script: bake @rpath/../lib (macos) / $ORIGIN/../lib (linux) / same-dir (windows) so relocatable install finds libopentui.
2. Add --native-lib bundling to install-oc2.sh: extract libopentui for the target triple into $INSTALL_DIR/../lib or beside binary; set rpath.
3. Add per-platform checksum manifest + SBOM generation (cargo-about/spdx-sbom) and a CHECKSUMS file pinning vendored .so SHA (computed locally, not copied from temp).
4. Fix native_launch.rs:268 path bug (x86_64-linux -> x86_64-unknown-linux-gnu) and the missing-triple path handling (macos arm64 dylib absent -> test must assert clean fail-closed, not panic on missing artifact).
5. Add macos/windows native artifact build+vendor step (requires Zig toolchain in CI image — local repo change to scripts/.github, external for actual runner).

### Human / external authority (cannot complete in-repo without external grant):
1. Native artifact rebuild for aarch64-apple-darwin, x86_64-apple-darwin, x86_64-pc-windows-msvc, aarch64-pc-windows-msvc: requires OpenTUI fork + Zig 0.16.0 + target SDKs. The genuine macOS dylib lives only in a disposable temp path (/private/var/folders/.../T/opencode/opentui-pinned/.../libopentui.dylib), evidence-only, NOT copied.
   - Genuine dylib: Mach-O arm64, SHA-256 de3564d496a92fe48fd6ca24a975b1b886cbb654ad5b7eea15bfe81afbf80b50, nm exports _createRenderer/_bufferDrawText/_bufferWriteResolvedChars/_destroyRenderer/_getCurrentRenderer.
2. Apple code-signing identity + notarization: NO codesign/notarytool/stapler step exists in repo. macOS arm64 build+sign+notarize+stapler requires Apple Developer ID, App Specific password, 2FA session — external grant. Missing signed libopentui.dylib = unsigned dylib; macOS Gatekeeper/Kernel will prompt/quit on load unless hardened+notarized.
3. Windows code-signing certificate: no .pfx/.pvk in repo; signed oc2.exe + signed opentui.dll require external cert.
4. Release artifact publishing: no GitHub Release workflow, no artifact index, no oc2_installer with embedded checksums. install-oc2.sh checksum arg is caller-supplied; no signed manifest.
5. Platform runner hardware: aarch64-apple-darwin notarized build needs macOS runner; windows needs windows-latest. Repo cannot self-provision these.

## 8. Serialized RED / implementation plan (one owned file per lane)

This lane is audit-only; it owns ONE file: worklog/TUI-011-PROD-AUDIT.md.
The following is a serialized plan for a follow-up implementation wave. Each lane = exactly one owned file. Tests are frozen; no product/test edits by this audit lane.

### Lane BLD-001 — Native artifact cross-compile + vendor (owned file: crates/opentui-bridge/build.rs + scripts/build-opentui.sh)
- RED: cargo test -p opencode-rk-opentui-bridge --features native --lib safe_renderer::tests::render_once_content_present -- --exact --test-threads=1  (FAILS: no aarch64-apple-darwin/libopentui.dylib in repo; build.rs panics "artifact missing")
- Impl: scripts/build-opentui.sh invokes pinned Zig to build libopentui for all 5 target triples; vendor into crates/opentui-bridge/native/lib/<triple>/. Add SHA-256 manifest scripts/build-opentui.sh.sha256.
- Verify: rtvk cargo test -p opencode-rk-opentui-bridge --features native --lib (GREEN). nm -gU on each artifact shows createRenderer/destroyRenderer/render.
- Locally solvable for build script; external for runner/toolchain.

### Lane BLD-002 — Relocatable rpath / loader path (owned file: crates/opentui-bridge/build.rs)
- RED: cargo build --release with --features native on macos (fails: dylib not found at runtime via dyld; no rpath)
- Impl: build.rs emits cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../lib (macos) / -Wl,-rpath,$ORIGIN/../lib (linux) / bundle dll beside exe (windows). Add install packaging to place libopentui in $INSTALL_DIR/../lib.
- Verify: otool -l oc2 | grep rpath (macos); readelf -d oc2 | grep RUNPATH (linux); dumpbin /dependents (windows).

### Lane BLD-003 — Installer native-lib bundling + rpath (owned file: scripts/install-oc2.sh)
- RED: ./scripts/install-oc2.sh --archive x.tar.gz --checksum S --install-dir /tmp/oc2-inst/oc2 --version v1; ldd /tmp/oc2-inst/oc2 | grep opentui (FAILS: not bundled)
- Impl: install-oc2.sh extracts libopentui.<triple> from archive stage into $INSTALL_DIR/../lib (unix) or beside oc2 (windows); sets nothing at runtime (rpath already baked by BLD-002).
- Verify: same; oc2 --version still identifies as oc2 (exit 74 gate intact).

### Lane BLD-004 — Checksum manifest + SBOM (owned file: scripts/release-artifact-metadata.sh)
- RED: repo has no CHECKSUMS file; no SBOM.
- Impl: scripts/release-artifact-metadata.sh generates checksums.txt (sha256 of oc2 + libopentui per platform) + sbom.json/spdx (cargo-about or syft). Pin vendored libopentui.so SHA-256.
- Verify: shasum -c checksums.txt; sbom.json has no proprietary-font entry; license scan shows MIT+Zig-runtime.

### Lane BLD-005 — macOS notarize + Windows sign (owned file: .github/workflows/release-sign.yml — external authority)
- RED: oc2 on macOS arm64 fails Gatekeeper; no notarization ticket.
- Impl: release-sign.yml runs codesign --timestamp --options runtime --identifier --sign "$APPLE_CERT" then notarytool submit; Windows SignTool sign /fd sha256 /f cert.pfx.
- Blocked: requires Apple Developer ID + App-specific password + 2FA (external authority). Human must provision secrets.VERIFIED_PATH in CI config.

### Lane TST-001 — Fix native_launch.rs artifact path (owned file: crates/cli/tests/native_launch.rs)
- RED: cargo test -p opencode-rk-cli --test native_launch (Scenario B path bug; panics on TTY guard before artifact check; line 268 wrong triple dir).
- Note: tests frozen; this lane is NOT this audit lane. Only document the gap here. Fix requires test-author role (not implementer).

## 9. Blocking summary (why not prod-ready)

- BLOCKED: No aarch64-apple-darwin / x86_64-apple-darwin / windows libopentui (external: Zig toolchain + runner).
- BLOCKED: No code-signing identity / notarization (external: Apple/Microsoft developer accounts).
- BLOCKED: install-oc2.sh does not bundle libopentui (missing implementation, locally solvable but lane not claimed).
- BLOCKED: no SBOM/checksum manifest for release artifacts (missing implementation, locally solvable).
- BLOCKED: native_launch.rs:268 path bug + TTY-guard ordering (frozen test, needs test-author role).
- BLOCKED: no relocatable rpath (build.rs does not set loader path).

Per CONVERGENCE.md and install_commands.rs contract, prod-ready requires: bare oc2 -> 1 authenticated daemon -> native OpenTUI frame -> tool execute -> persist -> exit -> restart -> resume -> second client — all on an installed binary under a fresh HOME. Current install-oc2.sh alone cannot produce a runnable native install on any non-Linux-x64 target. NOT prod-ready.

## 10. Commands to reproduce (audit-only, bounded, no heavy runs)

1. rtvk cat crates/opentui-bridge/build.rs (verify artifact gate)
2. rtvk find crates/opentui-bridge/native -type f (list vendored artifacts)
3. rtvk file crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu/libopentui.so (ELF x86-64, not stripped, debug_info)
4. rtvk shasum -a 256 /private/var/folders/.../T/opencode/opentui-pinned/packages/native/lib/aarch64-macos/libopentui.dylib (evidence-only genuine dylib; NOT copied)
5. rtvk grep -n "native\|libopentui" scripts/install-oc2.sh (no bundling)
6. rtvk grep -n "libopentui\|native/lib" crates/cli/tests/native_launch.rs (path bug line 268)
7. rtvk python3 tools/completion_claims.py (claim ledger)
8. rtvk python3 tools/lane_gate.py (gate status per file — TUI-011 has no Rust gate target; audit lane gate = scratchpad presence + ledger)