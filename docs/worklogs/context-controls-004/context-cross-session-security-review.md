# Security review

Verdict: NEEDS_FIX

Candidate SHA-256:
- `crates/storage/src/lib.rs`: `8c31cb3be5edd0ff14a7d4debb39ba21419860c7eb89f697a6239d39ea3cd5c7`
- `crates/storage/src/session_context.rs`: `a311af54a6ca7af09080badcbd6c5b6aea64b7510a6a7ddb977359b9ae4f5fad`
- `crates/storage/tests/context_cross_session_integrity.rs`: `34f2201db9f97da5d61ab91260488ab5a8c417678eec0ca951ed497809353f21`

Validated: raw/context bounded-history checks run before boundary/exclusion filtering; ownership query checks both round-turn and round-display ownership from either endpoint session. Preview validates within a deferred read transaction, then reads only two 4-KiB slices. Validator selects no payload.

Blockers: `Storage::display_tool_history` (`lib.rs:902`) remains an unvalidated current payload reader. If a round in A points to B's turn while its typed records point to A tool displays, its query returns A payload despite the cross-session owner-turn link. Add ownership validation there or prove it is not a reader in the affected security contract. Validator `OR` predicates span joined tables, so healthy-database checks can scan global round/record metadata; bounded `EXISTS` output does not bound examined rows. No payload is selected by this check, but metadata work is not per-session bounded. Scope through indexed per-session branches. Regression lacks display-reader, exclusion-hidden corruption, and foreign-turn cases. Compiled RED confirms raw-history and preview payload exposure before this fix.
