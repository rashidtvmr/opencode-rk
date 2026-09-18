# Worklog PAR-001-evidence

Claim: wrote owned file `sources/completion/legacy-evidence.json` with 258 legacy tasks mapped to audit shards, status not-evidence, TBD and empty-deps flags, repair lanes.

Source evidence (HEAD 5af7884cf7637c0760d985da7a03f2f99ccd0c78):
- `ralph.json` 258 userStories, all status accepted; `requirements/user-requirements.json` task refs.
- `ralph.completion.json:legacyPlan/legacyRequirements/auditShards/auditTests` + shard legacyPrefixes; `tasks/completion/parity.json:4` PAR-001 owns legacy-evidence.json + discovered.json.
- `/tmp/opencode/aud.json` auditAssignments (20 shards, 258 uniq, verified flat-minus-legacy empty both ways), tbdStories 82, emptyDependencies 237 — both re-verified equal to ralph.json-derived sets.
- `tools/completion_plan.py:legacy_report` owner rule (unknown prefix -> AUD-020); `--audit-legacy` output cross-checked.
- Shard assignedLegacy in `sources/completion/audits/AUD-*.json` read for repair-lane cross-check; per-shard repairChildren used for lane rule.
- AGENTS.md, docs/TDD.md, docs/SECURITY.md, PLAN.md read; no product code touched, no verifier/config edits.

Observed scenario: generated tasks array sorted by id; per-shard counts asserted equal to aud.json assignments before write.

Target boundary: owned file ONLY `sources/completion/legacy-evidence.json`. No edits to audits, parity.json, tools, ralph files.

Tests:
- `timeout 30 python3 -c "import json; d=json.load(open('sources/completion/legacy-evidence.json')); assert len(d['tasks'])==258; print('ok 258')"` -> ok 258.
- TBD set equality vs aud.json: True, 82. Empty-deps equality: True, 237. Shard assignment equality: True. Status flags: 258 accepted + statusIsReleaseEvidence false, 0 violations.
- `python3 tools/completion_plan.py --check` -> SPEC OK additions=90 legacy=258; NOT product acceptance.

Decisions:
- Repair lane = PAR lane consuming the shard per parity.json deps (AUD-002/008->PAR-002, AUD-003/007/015->PAR-003, AUD-005/009->PAR-005; AUD-004->PAR-004, AUD-006->PAR-006, AUD-012->PAR-007, AUD-014->PAR-008, AUD-010->PAR-009); shards with no PAR consumer (AUD-001/011/013/016/017/018/019/020) fall back to PAR-001 generic repair.
- AUD-020 holds 0 with catch-all unknown-prefix rule recorded in unknownPrefixRule.

Remaining unknowns: repair lanes are routing assignments, not completion proof; re-verification of all 258 per AUD-020 repairChildren still pending; discovered.json is sibling lane's file, untouched.
