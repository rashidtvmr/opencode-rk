# BRIDGE-046 sync_store

Claim: crates/opentui-bridge/src/sync_store.rs.
Source: data.tsx:124-403 handleEvent; :126-131 catalog.updated; :392-393 reference.updated; :395-401 integration.updated; :50-52 locationKey; :464-548 location stores. Reuse: context_session.rs:94-122 SyncState stale latch (no redefine Route/SyncState).
Observed: TS typed V2Event + network refresh; here stringly event+payload, location kinds only, stale latch.
Target: LocationStore entries bound 512, apply->Result unknown-event err, last_seq u64, stale per SyncState.
Tests: 6 fn (known/unknown/kind-bound/payload-bound/evict/stale-latch).
Decisions: stringly typed documented divergence; apply checks bounds before dispatch; evict oldest keep seq monotonic; SyncState new/stale reused.
Unknowns: lib.rs wiring by integrator (own one file only, no edit).
