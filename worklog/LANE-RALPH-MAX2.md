# LANE-RALPH-MAX2 scratchpad
claim: LANE-RALPH-MAX2 in-progress, session ses_f423cceb8ffeWknMXoafTgKEZd
source: tools/ralph_loop.py:64-65 DEFAULT maxConcurrentLanes=6; config/controller.settings.json maxConcurrentLanes=6 overrides default via load_settings
observed: 6 lanes + per-lane workers OOM (repo rev 6b19524)
target: cap effective lanes to 2 in tools/ralph_loop.py only (default + hard clamp, config override neutralized)
tests: py_compile only (no frozen Rust tests for this file)
decision: DEFAULT 6->2; clamp limit=min(limit,2) in main() so config/CLI cannot exceed 2
unknowns: none
