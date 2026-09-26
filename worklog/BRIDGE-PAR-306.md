# BRIDGE-PAR-306 scratchpad
claim: BRIDGE-PAR-306 ses_par306
evidence: stash.tsx:9-15 StashEntry/MAX_STASH_ENTRIES=50 (JSONL persisted); target simplified Vec<String> cap16/4KiB
tests: 4 (lifo, cap16, blank, trunc)
verify: rustfmt --check PASS, 85 lines
