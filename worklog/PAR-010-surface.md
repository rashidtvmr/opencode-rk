# PAR-010 surface-evidence worklog

Claim: differential-gate scaffold. 32 DISC-002 families → rows. No implemented claims.

Source evidence:
- HEAD 5af7884cf7637c0760d985da7a03f2f99ccd0c78.
- tasks/completion/parity.json:11-13 (PAR-010 journey/tests).
- sources/inventory/manifest.json: opencode 6626 entries @95daf90, 9router 1570 @17c4cc7.
- tools/reconcile_surfaces.py:53-102 fail-closed validator; 32 rules, 32 reviewed, 0 queued, all reviewState partial (24 impl partial, 8 reference-implemented).
- sources/completion/audits/AUD-019.json: inventory verified, vendored checkout + OpenTUI fork + dev-delta missing.
- AUD-001..020 finding classes: missing 27, verified 23 (tooling/existence only), unwired 11, unverified 7. All verdicts audit-complete, no acceptance.

Target boundary: owned file only sources/completion/surface-evidence.json.

Tests: `timeout 30 python3 -c "import json; d=json.load(open('sources/completion/surface-evidence.json')); print(len(d['surfaces'])); assert d['certified']==False"` → 32, pass.

Decisions:
- DISC-003 partial→unwired (19), reference-implemented→unverified (8 9router rows), plus opencode.contracts-schema + opencode.legacy-compatibility→unverified (spec/compat, no callable trace), opencode.clients-ui/opencode.desktop-client/opencode.enterprise-remote→missing (no crate/approved runtime, excluded lane).
- unverified total 10, missing 3, unwired 19. Zero implemented.
- Dev-delta 5a83358 adoption/defer TBD; OpenTUI c01292f absent → blocker.

Remaining unknowns: per-file dynamic enumeration (routes/commands/tools/config/events) needs pinned-tree pass; entrypoint traces + executable tests are PAR-002..009 work.
