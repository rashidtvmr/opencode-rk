# MCP config test compiler maintenance

Package: V2-MCP-CONFIG-COMPILER (independent test-owner mechanical maintenance)
Base: `de7e05fefba5374246078674432dcba07b0b5ff2`.

The fresh canonical workspace gate recorded by the requester fails during compilation at `crates/tools/tests/mcp_config.rs` lines 76 and 87: Rust repetition syntax was embedded inside `json!`, and line 76 additionally referenced an undefined `i`. This maintenance replaces those expressions with real Rust vectors of exactly 100 generated arguments and 10 cloned long arguments, respectively, preserving the bound scenarios and assertions. It imports the already-used `HashMap`. No semantic assertion, test name, product file, manifest, or lockfile is modified.

Verification is limited to the exclusive focused Cargo gate specified by the requester. Any runtime semantic failure will be recorded without changing assertions. This is a CANDIDATE only; acceptance requires the parent integrator's verification on the integrated revision.
