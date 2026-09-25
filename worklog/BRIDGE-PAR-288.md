# BRIDGE-PAR-288 scratchpad

claim: BRIDGE-PAR-288 via cc.claim, session ses_par288. OK.
source: `packages/tui/src/util/path.ts` (12 lines, win32-only normalizePath); home-tilde idiom from `path_format_ctx.rs:format_relative` + `runtime.tsx:abbreviateHome`.
target: new file `crates/opentui-bridge/src/path_util_full.rs`, std-only, forbid(unsafe_code), <100 lines.
api: `short_path(path,&str,home)->String` (home prefix -> ~, boundary-safe, 256-char cap); `base_name->&str` (after last /, trailing-slash tolerant); `parent_of->&str` (dir above, "/" sticky).
tests: tilde_hit, boundary_reject, basename, parent (+cap inside tilde test).
decisions: boundary-safe strip (no /home/u2 -> ~/2 false hit); "/" basename "/" parent "/"; char-based cap (unicode-safe).
