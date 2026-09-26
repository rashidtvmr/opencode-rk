# BRIDGE-SPIN-FIX worklog

Claim: attempted via tools/completion_claims.py; CLI usage unclear (help prints ledger summary only). Proceeding per task instruction, claim failure noted.

Fix: test-only, no impl change. `hold_positions_stable` line 188 expected bright head at late hold; TS `calculateColorIndex` (spinner.ts:122-126) returns dist+holdProgress while holding, glyph map (spinner.ts:313-318) sends out-of-range index to inactive '·'. End-hold last frame: idx 0+8=8 >= TRAIL_LEN 6 -> '·'. Corrected both late-hold assertions to '·', kept first-frame head assertions, documented in module docs with TS line refs.

Verification: cargo not run per scope; green by inspection against TS math.
