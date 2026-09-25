# BRIDGE-PAR-298
Claim: ses_par298, in-progress (pre-held by orchestrator).
Source: packages/tui/src/util/system.ts (describeOS darwin/win32/linux+arch; describeTerminal TERM_* env); style ref crates/opentui-bridge/src/transcript_util_full.rs.
Target: crates/opentui-bridge/src/system_util_full.rs only. No lib.rs/Cargo.toml edits, no cargo, no commit.
API: plat()->&'static str via cfg (windows/macos else linux); arch_label()->&'static str via cfg (x86_64->x64, aarch64->arm64 else other); shell_of()->&'static str from $SHELL or cfg fallback (cmd/sh). std-only, forbid(unsafe_code), <90 lines.
Tests: 6 (plat_is_known, arch_is_known, fallback_is_known, shell_of_nonempty + cfg-gated plat_linux, arch_x64).
Verify: rustfmt --check only.
