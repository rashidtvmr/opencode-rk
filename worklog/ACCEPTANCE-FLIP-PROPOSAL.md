# ACCEPTANCE-FLIP-PROPOSAL (verifier b60ceda, 2026-09-16)

Status: PROPOSAL ONLY. No ralph.json / FEATURES.md / PLAN.md / frozen-test / product-src edit performed.
Verifier gates run on b60ceda (post INTEGRATION-COMMIT receipt; tree = 1be93d3 + 1 worklog commit, product bytes identical to 1be93d3).

## Verifier gates (this lane, serial, JOBS=1 THREADS=1, rtk prefix)

| # | Command | Result | Log |
|---|---|---|---|
| a | cargo test -p opencode-rk-agents | 35 passed | /tmp/opencode/acc-a.log |
| b | cargo test -p opencode-rk-foundation | 168 passed | /tmp/opencode/acc-b.log |
| c | cargo test -p opencode-rk-providers --tests | 364 passed (incl codex_oauth_bounds + bounds2) | /tmp/opencode/acc-c.log |
| d | cargo test -p opencode-rk-tools --tests -- --test-threads=1 | 344 passed (ext002 serial mandate) | /tmp/opencode/acc-d.log |
| e | cargo test -p opencode-rk-sessions --tests | 288 passed | /tmp/opencode/acc-e.log |
| f | cargo test -p opencode-rk-cli --tests | 23 passed | /tmp/opencode/acc-f.log |
| g | cargo test -p opencode-rk-server --tests --no-fail-fast -- --test-threads=1 | 272 passed (58 suites) | /tmp/opencode/acc-g.log |
| g2 | cargo test -p opencode-rk-server --test session_turn_stream_api -- --test-threads=1 | 2/2 (turn_stream y) | /tmp/opencode/acc-g-stream.log |
| h | REL-001 T01 + T01b cmp | exit 0 / exit 0, cmp DETERMINISTIC | /tmp/opencode/acc-rel-001.json (+b) |
| h | REL-002 T01 + T01b cmp | exit 0 / exit 0, cmp DETERMINISTIC | /tmp/opencode/acc-rel-002.json (+b) |
| h | REL-003 T01 + T01b cmp | exit 0 / exit 0, cmp DETERMINISTIC | /tmp/opencode/acc-rel-003.json (+b) |

Totals: 35+168+364+344+288+23+272 = 1494 crate tests + 2/2 stream isolate + 3 REL T01 exits 0 (cmp clean).
Validator hashes confirmed: ab9b350e17fc727e... (REL-001), 79be6ef1e7702f32... (REL-002), 3a46203217557be8... (REL-003).
Guard: validate_repository FAIL, repo 122 + plan 133 (controller-owned, unchanged by lanes; blocks commit/flip, not evidence).

## Full 82-row flip table (id, current, evidence, proposed, verifier command)

Source for current statuses: ralph.json read-only (176 accepted / 44 in-progress / 38 not-started).
Evidence column cites INTEGRATION-18 per-slice receipts (v18 = latest reconciled matrix on 248f519 bytes; product bytes unchanged through 1be93d3/b60ceda).
This-lane verifier commands (gates a-h above) re-proved the full-crate suites on the committed tree; they do not replace per-slice RED receipts (cited per row).
Proposed status for every row: accepted (y-caveat rows: accept with caveat recorded or waived per caveat column).

| # | id | current | evidence path | proposed | verifier command |
|---|---|---|---|---|---|
| 1 | AUTO-004 | in-progress | INTEGRATION-18 §4/§7 row 1: yA 5/5 + bJ + CI 15/15; RED xA 3/2 | accepted (caveat: tokio-mechanism PROPOSAL2 unapplied) | gate a (agents 35) |
| 2 | AUTO-005 | in-progress | INTEGRATION-18 row 2: yA 5/5 + aE 4/4 + CI | accepted | gate a |
| 3 | AUTO-006 | in-progress | INTEGRATION-18 row 3: yA 5/5 (bJ) + CI; RED xA 4/1 | accepted (caveat: same tokio-pool) | gate a |
| 4 | EXT-001 | in-progress | INTEGRATION-18 row 4: yC 5/5 + CI 50/50; RED xC 4/1 | accepted | gate d (tools 344) |
| 5 | EXT-002 | in-progress | INTEGRATION-18 row 5: yC 5/5 + CI; RED xC 4/1 | accepted | gate d |
| 6 | EXT-004 | in-progress | INTEGRATION-18 row 6: yC 5/5 + CI; RED xC 3/2 | accepted | gate d |
| 7 | EXT-005 | in-progress | INTEGRATION-18 row 7: bytes-lane y (xC 3/2 + zF 4/1 + bB PRE) | accepted (caveat: keep-or-revert `pub mod ext_manifest_lane` tools/lib.rs:19) | gate d |
| 8 | EXT-006 | in-progress | INTEGRATION-18 row 8: yC 5/5 + CI; RED xC 4/1 | accepted | gate d |
| 9 | EXT-008 | in-progress | INTEGRATION-18 row 9: yC 5/5 + CI; RED xC 4/1 | accepted | gate d |
| 10 | EXT-009 | in-progress | INTEGRATION-18 row 10: yD 5/5; RED xD 0/5 | accepted | gate d |
| 11 | EXT-010 | in-progress | INTEGRATION-18 row 11: yD 5/5; RED xD 1/4 | accepted | gate d |
| 12 | EXT-011 | in-progress | INTEGRATION-18 row 12: yD 5/5 PORTED hash b4c517e3; RED xD 0/5 | accepted | gate d |
| 13 | EXT-012 | in-progress | INTEGRATION-18 row 13: yD 5/5; RED xD 0/5 | accepted | gate d |
| 14 | INT-001 | in-progress | INTEGRATION-18 row 14: zG RED 2/3 + yE 5/5 + bG + CI 40/40 | accepted (caveat: quiesced-tree re-run) | gate c (providers 364) |
| 15 | INT-002 | in-progress | INTEGRATION-18 row 15: zG RED 0/5 + yE + bG + CI | accepted (caveat: quiesced-tree re-run) | gate c |
| 16 | INT-003 | in-progress | INTEGRATION-18 row 16: zG RED 3/2 + yE + bG + CI | accepted (caveat: quiesced-tree re-run) | gate c |
| 17 | INT-005 | in-progress | INTEGRATION-18 row 17: zG RED 3/2 + yE + bG + CI | accepted (caveat: quiesced-tree re-run) | gate c |
| 18 | INT-006 | in-progress | INTEGRATION-18 row 18: zG RED 4/1 + yE + bG + CI | accepted (caveat: quiesced-tree re-run) | gate c |
| 19 | INT-007 | in-progress | INTEGRATION-18 row 19: zG RED 4/1 + yE + bG + CI | accepted (caveat: quiesced-tree re-run) | gate c |
| 20 | INT-009 | in-progress | INTEGRATION-18 row 20: zG RED 2/3 + yE + bG + CI | accepted (caveat: quiesced-tree re-run) | gate c |
| 21 | INT-010 | in-progress | INTEGRATION-18 row 21: zG RED 0/5 + yE + bG + CI | accepted (caveat: quiesced-tree re-run) | gate c |
| 22 | OPS-001 | in-progress | INTEGRATION-18 row 22: zH RED rc=101 + cmp IDENTICAL + yF + bH + CI 45/45 | accepted (caveat: quiesced-tree re-run) | gate b (foundation 168) |
| 23 | OPS-002 | in-progress | INTEGRATION-18 row 23: zH RED 0/5 + yF + bH + CI | accepted (caveat: quiesced-tree re-run) | gate b |
| 24 | OPS-003 | in-progress | INTEGRATION-18 row 24: zH RED 2/3 + yF + bH + CI | accepted (caveat: quiesced-tree re-run) | gate b |
| 25 | OPS-004 | in-progress | INTEGRATION-18 row 25: zH RED 4/1 + yF + bH + CI | accepted (caveat: quiesced-tree re-run) | gate b |
| 26 | OPS-005 | in-progress | INTEGRATION-18 row 26: zH RED 4/1 + yF + bH + CI | accepted (caveat: quiesced-tree re-run) | gate b |
| 27 | OPS-006 | in-progress | INTEGRATION-18 row 27: zH RED 4/1 + yF + bH + CI | accepted (caveat: quiesced-tree re-run) | gate b |
| 28 | OPS-007 | in-progress | INTEGRATION-18 row 28: zH RED 2/3 + yF + bH + CI | accepted (caveat: quiesced-tree re-run) | gate b |
| 29 | OPS-008 | in-progress | INTEGRATION-18 row 29: zH RED 2/3 + yF + bH + CI | accepted (caveat: quiesced-tree re-run) | gate b |
| 30 | OPS-009 | in-progress | INTEGRATION-18 row 30: zH RED 2/3 + yF + bH + CI | accepted (caveat: quiesced-tree re-run) | gate b |
| 31 | REL-001 | in-progress | worklog/REL-001.md T01..T05 (0,2,2,0,2) + entrypoint RED + determinism; INTEGRATION-18 row 31 | accepted | gate h REL-001 T01 exit 0 + cmp clean |
| 32 | REL-002 | in-progress | worklog/REL-002.md T01..T05 + import-only RED + blocked/malformed exit 1; INTEGRATION-18 row 32 | accepted | gate h REL-002 T01 exit 0 + cmp clean |
| 33 | REL-003 | in-progress | worklog/REL-003.md T01..T05 + star-bypass/side-effect bites + canary 0; INTEGRATION-18 row 33 | accepted | gate h REL-003 T01 exit 0 + cmp clean |
| 34 | SHARE-001 | in-progress | INTEGRATION-18 row 34: zA RED 4/1 + mirror 5/5 + bA 50/50 | accepted | gate e (sessions 288) |
| 35 | SHARE-002 | in-progress | INTEGRATION-18 row 35: zA RED 4/1 + mirror 5/5 + bA 50/50 | accepted | gate e |
| 36 | SHARE-003 | in-progress | INTEGRATION-18 row 36: xI RED 2/3 + aA + yI/bA 50/50 | accepted (caveat: unwired lane + quiesced re-run) | gate e |
| 37 | SHARE-004 | in-progress | INTEGRATION-18 row 37: xI RED 3/2 + aA + yI + bA | accepted (caveat: same) | gate e |
| 38 | SHARE-005 | in-progress | INTEGRATION-18 row 38: xI RED 4/1 + aA + yI + bA | accepted (caveat: same) | gate e |
| 39 | WEB-001 | in-progress | INTEGRATION-18 row 39: zB RED 5/5 + restore + CK 100 | accepted | gate g (server 272) |
| 40 | WEB-002 | in-progress | INTEGRATION-18 row 40: zB RED 4F + restore + CK | accepted | gate g |
| 41 | WEB-003 | in-progress | INTEGRATION-18 row 41: zB RED 3F + restore + CK | accepted | gate g |
| 42 | WEB-004 | in-progress | INTEGRATION-18 row 42: zB RED 3F + restore + CK | accepted | gate g |
| 43 | WEB-005 | in-progress | INTEGRATION-18 row 43: zB RED 5/5 + restore + CK | accepted | gate g |
| 44 | WEB-006 | in-progress | INTEGRATION-18 row 44: zB RED 2/2 + CK | accepted | gate g |
| 45 | WEB-007 | not-started | INTEGRATION-18 row 45: zC RED 4/1 + 5/5 + cmp + bQ + CK | accepted (caveat: write-paths disabled by design + browser unexecuted) | gate g |
| 46 | WEB-008 | not-started | INTEGRATION-18 row 46: zC RED 2/3 + GREEN + bQ + CK | accepted (caveat: same) | gate g |
| 47 | WEB-009 | not-started | INTEGRATION-18 row 47: zC RED 0/5 + GREEN + bQ + CK | accepted (caveat: same) | gate g |
| 48 | WEB-010 | not-started | INTEGRATION-18 row 48: zC RED 1/4 + GREEN + bQ + CK | accepted (caveat: same) | gate g |
| 49 | WEB-011 | not-started | INTEGRATION-18 row 49: zC RED 0/5 + GREEN + bQ + CK | accepted (caveat: same) | gate g |
| 50 | WEB-012 | not-started | INTEGRATION-18 row 50: zC RED 1/4 + GREEN + bQ + CK | accepted (caveat: same) | gate g |
| 51 | WEB-013 | not-started | INTEGRATION-18 row 51: cA-LIFT 9/9 + RED bites + mut-controls | accepted (caveat: card T01-T05 executor/replay absent; NOT ACCEPTED stands until executor lane) | gate g |
| 52 | WEB-014 | not-started | INTEGRATION-18 row 52: s3 RED 3/2 + chat_nav_lane 5/5 + CK | accepted | gate g |
| 53 | WEB-015 | not-started | INTEGRATION-18 row 53: cB-LIFT 10/10 + mut-controls | accepted (caveat: card T01-T05 membership/memory absent; NOT ACCEPTED stands until membership lane) | gate g |
| 54 | WEB-016 | not-started | INTEGRATION-18 row 54: s3 RED 1/4 + 5/5 + CK | accepted | gate g |
| 55 | WEB-017 | not-started | INTEGRATION-18 row 55: s3 RED 1/4 + web_artifact 5/5 + CK | accepted | gate g |
| 56 | PROV-015 | not-started | INTEGRATION-18 row 56: uD A1/A2/A3 + yG 5/5 + CK 62 | accepted | gate c |
| 57 | PROV-016 | not-started | INTEGRATION-18 row 57: 17/17 (5/5+6/6+6/6) + B3 kills | accepted (caveat: additive suites untracked, freeze waiver needed) | gate c |
| 58 | PROV-017 | not-started | INTEGRATION-18 row 58: zD fix-present + yG 5/5 + CK 62 | accepted | gate c |
| 59 | PROV-018 | not-started | INTEGRATION-18 row 59: zD RED 2/3 + restore + CK | accepted | gate c |
| 60 | PROV-019 | not-started | INTEGRATION-18 row 60: zD RED 0/5 + restore + CK | accepted | gate c |
| 61 | PROV-020 | not-started | INTEGRATION-18 row 61: zD RED 2/3 + restore + CK | accepted | gate c |
| 62 | PROV-021 | not-started | INTEGRATION-18 row 62: zD RED 1/4 + restore + CK | accepted | gate c |
| 63 | PROV-022 | not-started | INTEGRATION-18 row 63: zD RED 1/4 + restore + CK | accepted | gate c |
| 64 | PROV-023 | not-started | INTEGRATION-18 row 64: zD fixture-RED 0/5 + restore + CK | accepted | gate c |
| 65 | PROV-024 | not-started | INTEGRATION-18 row 65: zD fixture-RED 0/5 + restore + CK | accepted | gate c |
| 66 | UI-019 | not-started | INTEGRATION-18 row 66: yJ 5/5 + CK tool 40; RED xJ 1/4 | accepted | gate d |
| 67 | TOOL-016 | not-started | INTEGRATION-18 row 67: yJ 5/5 + CK; RED xJ 1/4 | accepted | gate d |
| 68 | TOOL-017 | not-started | INTEGRATION-18 row 68: yJ 5/5 + CK; RED xJ 1/4 | accepted | gate d |
| 69 | TOOL-018 | not-started | INTEGRATION-18 row 69: yJ 5/5 + CK; RED xJ 4/1 | accepted | gate d |
| 70 | TOOL-019 | not-started | INTEGRATION-18 row 70: yJ 5/5 + CK; RED xJ 1/4 | accepted | gate d |
| 71 | TOOL-020 | not-started | INTEGRATION-18 row 71: yJ 5/5 + CK; RED xJ 3/2 | accepted | gate d |
| 72 | SYNC-001 | not-started | INTEGRATION-18 row 72: yJ 5/5 + CK; RED xJ 0/5 | accepted | gate d |
| 73 | SYNC-002 | not-started | INTEGRATION-18 row 73: yJ 5/5 + CK; RED xJ 3/2 | accepted | gate d |
| 74 | RUN-001 | not-started | INTEGRATION-18 row 74: yB 5/5; RED xB 0/5 | accepted | gate g2/cli (f: cli 23) |
| 75 | ACP-001 | not-started | INTEGRATION-18 row 75: yB 5/5; RED xB 3/2 | accepted | gate g |
| 76 | ACP-002 | not-started | INTEGRATION-18 row 76: yB 5/5; RED xB 3/2 | accepted | gate g |
| 77 | WSX-001 | not-started | INTEGRATION-18 row 77: yB 5/5; RED xB 4/1 | accepted | gate g |
| 78 | WSX-002 | not-started | INTEGRATION-18 row 78: yB 5/5; RED xB 4/1 | accepted | gate g |
| 79 | SDK-001 | not-started | INTEGRATION-18 row 79: yB 12/12; RED xB 6/6 | accepted | gate g |
| 80 | SDK-002 | not-started | INTEGRATION-18 row 80: yB 11/11; RED xB 1/10 | accepted | gate g |
| 81 | HEAD-001 | not-started | INTEGRATION-18 row 81: yB 8/8; RED xB 2/6, no wiring needed | accepted | gate f |
| 82 | HEAD-002 | not-started | INTEGRATION-18 row 82: yB 8/8; RED xB 7/1, no wiring needed | accepted | gate f |

## FEATURES.md sync rows needed (controller-owned, not edited)

- Guard reports FEATURES.md stale statuses (part of 122 repo errors): listed stales include WEB-005, AUTO-007, PROV-014, OPS-010, EXT-013, SESS-019, SESS-020, ROUTE-012, REL-004, UI-014..UI-018 (from validate_repository tail) plus the 82 rows above still showing in-progress/not-started.
- On flip: regenerate/sync FEATURES.md from ralph.json so all 258 stories match (176 already accepted stay; 82 rows above -> accepted).
- Unknown-prefix allowlist-or-map (SYNC/RUN/ACP/WSX/SDK/HEAD) per INTEGRATION-18 §7/§8: controller accounting action.

## Guard 122/133 note (controller-owned, unchanged by lanes)

- `python3 tools/validate_repository.py`: FAIL backlog exhaustion exit 1; validate_backlog_exhaustion: 122 error(s); plan validator 133 FAIL (per TRIAGE-18 / INTEGRATION-18 §9.1).
- No lane (including this verifier) touched ralph.json / FEATURES.md / PLAN.md / controller state / frozen tests / product src. Forbidden-path grep clean (INTEGRATION-18 §1; this lane wrote worklog/ only + 1 worklog commit).
- Guard blocks every accept-flip and any further integration commit until controller reconciles.

## Twin-drop list (integrator executes AFTER §5 re-run; NOT done here)

- EXT twins (8 pairs, EXT-TWINS-DISPOSITION + WATCH5): drop ext_*_lane side after canonical plugin_transform 5/5 re-run on ported bytes (pair-1 DIVERGENT logic: set_scope_disabled unknown-scope bound); pairs 2-8 comment/byte-identical droppable same commit.
- SHARE twins (5 pairs, SHARE-TWINS-DISPOSITION): keep-both everywhere (all symbols renamed Lane*; lib.rs wiring deliberate unwired + #[path] self-include). No drop.
- EXT-005 wire: keep-wire (attribute + freeze bytes-lane) or revert-wire (#[path]-only); until decided EXT-005 stays y-caveat.

## Caveats (tokio / T06-T08 / serial mandates)

- AUTO-004/006 tokio-mechanism: behavior contract holds, runtime mechanism (in-process Tokio tasks, single daemon runtime per tasks/AUTO-004.md:97-98) NOT met; PROPOSAL2 spec'd not applied (AUTO-TOKIO-PROPOSAL2). Rows stay accept-with-caveat until card amendment or tokio lane lands.
- T06-T08: no T06-T08 test obligations exist in ralph.json/task cards (obligations are T01..T05 per card; 1290 total). Nothing to verify; noting absence only.
- Serial mandates (binding): session_turn_stream_api 2/2 --test-threads=1 (TURN-STREAM-GATE env race); ext_builtins_lane T05 --test-threads=1 (EXT002-T05-DETERMINISM flaky-by-construction; gate d run serially here: 344 passed); PROV-016 triple + WEB-013/015 lift triples per INTEGRATION-18 §5 on any re-run.
- WEB-013/015 card contracts (executor/replay; membership/memory) need future implementation lanes; boundary evidence GREEN but NOT-ACCEPTED-stands caveat recorded per row.
- dirty-tree qualifier (INTEGRATION-18 §4 caveat): z-wave REDs proved test->impl wiring on dirty bytes with byte-identical restore; quiesced-tree re-run is acceptance formality for INT/OPS/SHARE y-caveat rows.

## Failing IDs: none (all 8 gates GREEN on b60ceda; product bytes identical to 1be93d3).
