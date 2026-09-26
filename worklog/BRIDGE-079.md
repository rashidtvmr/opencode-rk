# BRIDGE-079 parsers_config

Claim: static URL table mirror of parsers-config.ts.
Source: /home/rashid/projects/opencode/packages/tui/src/parsers-config.ts:6-385 (34 parsers entries, aliases diff:[udiff,patch] make:[makefile]); renderer_config.rs:20-23 builtin langs excluded.
Target: ONLY crates/opentui-bridge/src/parsers_config.rs.
Tests: 5 (alias, unknown None, count==34, verbatim rust+nix URLs, no-empty-urls). Not run (no cargo per scope); logic trace green.
Decisions: query_url = first active highlights URL; locals/2nd highlight URLs + commented broken alternates omitted (documented). No fetch.
Unknowns: none.
