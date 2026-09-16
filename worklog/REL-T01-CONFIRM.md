# REL-T01-CONFIRM (cC verify, no product edit)

Rev `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`. Serial runs, timeout 120, rtk prefix. No validator/fixture/ralph.json edits (`git status --short -- tools/ fixtures/ ralph.json` clean).

## Hashes (confirm)
- `tools/check_release_accounting.py`: `ab9b350e17fc727e4d35d18dd30c7d468a7307ce2e0fa29fddf1a28a15554bda` (prefix ab9b350e OK)
- `tools/check_release_tdd.py`: `79be6ef1e7702f3224b378e2864f588a8eed5db1600fbd6917de8bf4071b9c04` (prefix 79be6ef1 OK)
- `tools/check_release_safety.py`: `3a46203217557be88135eafa5b9725cba3b1176aaf0c74a9d552b56bc5849b45` (prefix 3a462032 OK)

## REL-001 T01 x2
- Cmd: `python3 tools/check_release_accounting.py --snapshot fixtures/release-accounting/complete --out /tmp/opencode/cC-rel-001.json` (+ `cC-rel-001b.json`)
- Exits: 0, 0. Report passed true, missing [] misclassified [] partition release-feature-accounting-validator.
- Determinism: `cmp cC-rel-001.json cC-rel-001b.json` clean; sha256 `b6d34527...` both.

## REL-002 T01 x2
- Cmd: `python3 tools/check_release_tdd.py --revision 863a0019fc6f0b9779d867792fe00c4a9ab84c87 --manifest fixtures/release-tdd/frozen.json --receipts fixtures/release-tdd/receipts --gates fixtures/release-tdd/gates --out /tmp/opencode/cC-rel-002.json` (+b)
- Exits: 0, 0. All four checks pass, failing_fixture null, partition strict-tdd-independent-verification-validator.
- Determinism: `cmp` clean; sha256 `3600e645...` both.

## REL-003 T01 x2
- Cmd: `python3 tools/check_release_safety.py --revision 248f519d53ed29cbf7fef7ceb2b5010fb1a4224b --gates fixtures/release-safety/gates --caps fixtures/release-safety/caps.json --quotas fixtures/release-safety/quotas.json --out /tmp/opencode/cC-rel-003.json` (+b)
- Exits: 0, 0. All four checks pass, failing_fixture null, partition safety-resource-correctness-release-validator.
- Determinism: `cmp` clean; sha256 `238bc203...` both.
