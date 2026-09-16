# REL lane-gate receipts (bF verify, no product edit)

Rev `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`. lane_gate.py covers storage format-2 lanes only; no REL-001/002/003 lanes exist. Receipts = REL T01 triple rerun + determinism cmp + full failing-triple spot checks. No edits to validators/fixtures/ralph.json.

## lane_gate.py
- `python3 tools/lane_gate.py --help` rc=0 (`--json`, `--run` flags).
- `python3 tools/lane_gate.py --json` rc=1 (8 module lanes PASS; 4 test lanes UNRUN without `--run`; no REL lanes defined). Full JSON: `/tmp/opencode/bF-lanegate.json`.
- `--run` not executed (would compile/run storage Rust harnesses; out of REL scope, serial 120s budget).

## Validator hashes (confirm match)
- `ab9b350e17fc727e4d35d18dd30c7d468a7307ce2e0fa29fddf1a28a15554bda` tools/check_release_accounting.py
- `79be6ef1e7702f3224b378e2864f588a8eed5db1600fbd6917de8bf4071b9c04` tools/check_release_tdd.py
- `3a46203217557be88135eafa5b9725cba3b1176aaf0c74a9d552b56bc5849b45` tools/check_release_safety.py

## REL-001 (accounting)
- T01 complete exit 0 pass, empty missing/misclassified; report `/tmp/opencode/bF-rel-001.json`.
- T01b exit 0, `cmp` byte-identical DETERMINISTIC (`/tmp/opencode/bF-rel-001b.json`).
- T02 incomplete exit 2; T03 misclassified exit 2 (reports `bF-rel-001-t02/t03.json`).
- `git status --short` on tools/fixtures/ralph.json: clean.

## REL-002 (TDD, rev 863a0019fc6f0b9779d867792fe00c4a9ab84c87)
- T01 exit 0 all four checks pass; report `/tmp/opencode/bF-rel-002.json`.
- T01b exit 0, `cmp` byte-identical DETERMINISTIC (`/tmp/opencode/bF-rel-002b.json`).
- T02 fail-no-red exit 2; T03 fail-mutated exit 2 (reports `bF-rel-002-t02/t03.json`).

## REL-003 (safety, rev 248f519d53ed29cbf7fef7ceb2b5010fb1a4224b)
- T01 exit 0 all four checks pass; report `/tmp/opencode/bF-rel-003.json`.
- T01b exit 0, `cmp` byte-identical DETERMINISTIC (`/tmp/opencode/bF-rel-003b.json`).
- T02 fail-star-bypass exit 2 (report `bF-rel-003-t02.json`); canary grep count 0 in T01 report.

## Verdict
- REL T01 triple GREEN + determinism PASS for REL-001/002/003. lane_gate.py itself: no REL coverage (out-of-scope lanes: 8 PASS modules, 4 UNRUN tests without --run).
