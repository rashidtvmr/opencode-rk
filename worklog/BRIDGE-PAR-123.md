# BRIDGE-PAR-123 scratchpad
claim: in-progress ses_par123
evidence: packages/tui/src/component/prompt/index.tsx:1181 CRLF fold `text.replace(/\r\n/g,"\n").replace(/\r/g,"\n")`; index.tsx:29 normalizePromptContent import; lines 1-60 read.
boundary: ONE file crates/opentui-bridge/src/prompt_full.rs only. No lib.rs/Cargo.toml/composer/traits edits. No cargo/commit.
tests: 6 in-file (crlf, lone-cr, ansi-strip, slash, mention, trunc-cap).
decisions: char-based 8KiB trunc; CSI-only ANSI strip per spec; slash=trim-start starts-with-/; mention=contains-@.
