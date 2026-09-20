# LANE-AUTODRIVE-CLAMP

Claim: LANE-AUTODRIVE-CLAMP owned by ses_f423ccebbffe34G50hL9WgxgWo.
Source: tools/auto_drive.py:200 `--lanes default=15`; rev 6b19524.
Observed: 15 parallel workspace builds OOM vs 8GB budget.
Target: tools/auto_drive.py only. Default 2, cap 4, serialized verify.
Tests: `timeout 60 python3 -m py_compile tools/auto_drive.py` -> OK.
Decisions: DEFAULT_LANES=2, MAX_LANES=4 clamp with log, threading.Lock around validate_repository subprocess, ThreadPoolExecutor already bounded by lanes so one heavy verify at a time holds. Usage line updated to --lanes 2.
Unknowns: none.
