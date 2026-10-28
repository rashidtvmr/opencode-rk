# Title Projection v2 - Independent Review Verdict

## Verdict: APPROVED (with gap noted)

**Reviewer:** Title projection evaluator  
**Base SHA:** 6f6872bf87e02f05afe9c1a5f63e55fe4c040499 (canonical `crates/cli/src/native_navigation.rs` e15b0334...)  
**Candidate SHA:** 9b5498335cad7c45e2d540dd8864ec4e5f13ec55bdaa84ed55b3a373f12f9f48  
**Canonical `native_navigation.rs`:** e15b0334aefb4b7090ba4d8b4c532faa592f96d93de26a5bf7e65266ec126df2  
**Canonical `native_view_sessions.rs`:** 7bbe8d91542aa0f5b71ef707b07c30fc1c1003115aa980cb36050ba2d9ced5b2  
**Canonical `native_tabs.rs`:** 72ddf7edf665e2c4a987b6e8bdba3b8e63cbb792054acb081544b99227b25611  
**Canonical `tui_entry.rs`:** fcc2d0bb1d83c51defb80709e6d223c796b4d09f8519b54f4c41450716dcb31e  
**Upstream pin:** 450e7b92412c780a7b5a75226bb8d49aaca24e22  

## Evidence review against canonical source

### 1. Bounded snapshot titles (MAX_SNAPSHOT_TITLES = MAX_TABS = 32)
**Candidate claims:** Yes - adds `snapshot_titles: Vec<(SessionRef, String)>` bounded by `MAX_SNAPSHOT_TITLES = MAX_TABS = 32`.

**Canonical source confirmation:**
- `native_navigation.rs:28` `pub const MAX_TABS: usize = 32;` - confirmed
- `native_navigation.rs:30-31` `pub const MAX_SNAPSHOT_TITLES: usize = MAX_TABS;` - candidate adds this, canonical does NOT have it yet. This is new addition.
- `native_navigation.rs:299` `snapshot_titles: Vec<(SessionRef, String)>` - candidate adds this field, canonical does NOT have it. This is a new field.

**Verdict:** The candidate ADDS these. They do not exist in canonical yet. The constant and field are new insertions. Correct.

### 2. `ingest_snapshot_title` / `ingest_snapshot_titles` API
**Candidate claims:**
- Blocks archived sessions
- No revision fabrication
- Bounded at `MAX_SNAPSHOT_TITLES`

**Canonical source confirmation:**
- Canonical `native_navigation.rs:605-610` has `title()` returning ONLY from `titles` field. NO `snapshot_titles` logic.
- Canonical does NOT have `ingest_snapshot_title` or `ingest_snapshot_titles` methods at all.
- The candidate adds these methods at lines 614-641 of the candidate file.

**Verdict:** Candidate correctly ADDS these methods. The archive guard is present in canonical `apply()` (`tombstone_at_or_above`), but the snapshot ingest path is new.

### 3. `title()` precedence: revisioned titles THEN snapshot_titles
**Candidate claims:** `title()` reads `titles` first, then falls back to `snapshot_titles`.

**Canonical source confirmation:**
- Canonical `native_navigation.rs:605-610`:
  ```rust
  pub fn title(&self, session: &SessionRef) -> Option<&str> {
      self.titles
          .iter()
          .find(|(s, _, _)| s == session)
          .map(|(_, t, _)| t.as_str())
  }
  ```
  NO snapshot fallback exists in canonical.
- Candidate `native_navigation.rs:646-658` adds the fallback:
  ```rust
  pub fn title(&self, session: &SessionRef) -> Option<&str> {
      self.titles
          .iter()
          .find(|(s, _, _)| s == session)
          .map(|(_, t, _)| t.as_str())
          .or_else(|| {
              self.snapshot_titles
                  .iter()
                  .find(|(s, _)| s == session)
                  .map(|(_, t)| t.as_str())
          })
  }
  ```

**Verdict:** Candidate correctly ADDS the fallback. `Option<&str>` preserves `Some("")` vs `None` distinction - confirmed by test at candidate line 907: `assert_eq!(nav.title(&a), Some(""));`

### 4. `apply(Renamed)` supersedes snapshot titles
**Candidate claims:** An accepted revisioned rename removes matching `snapshot_titles` entries.

**Canonical source confirmation:**
- Canonical `native_navigation.rs:541-564` `apply(DaemonEvent::Renamed)` does NOT touch `snapshot_titles` - because that field does not exist in canonical.
- Candidate `native_navigation.rs:546-573` adds two `self.snapshot_titles.retain(...)` calls in both the `Some(entry)` and `None` branches of the rename handler.

**Verdict:** Correct. The retain calls are added alongside the existing revision-gated update logic. Stale renames (rev <= rev) are correctly dropped before reaching the retain.

### 5. `apply(Archived)` removes snapshot titles
**Candidate claims:** Archive event removes from `snapshot_titles`.

**Canonical source confirmation:**
- Canonical `native_navigation.rs:565-576` `apply(DaemonEvent::Archived)`:
  ```rust
  self.remove_tab(&session);
  self.titles.retain(|(s, _, _)| *s != session);
  self.work.retain(|(s, _)| *s != session);
  ```
  NO `snapshot_titles` retain - field doesn't exist in canonical.
- Candidate `native_navigation.rs:578-586` adds `self.snapshot_titles.retain(|(s, _)| *s != session);`

**Verdict:** Correct. The candidate adds the retain call correctly positioned after removing titles and work.

### 6. No resurrection of archived sessions via snapshot
**Candidate claims:** `ingest_snapshot_title` returns early if session is archived.

**Canonical source confirmation:**
- Candidate `native_navigation.rs:620-622`:
  ```rust
  if self.archived.iter().any(|(s, _)| *s == session) {
      return;
  }
  ```
  This correctly checks the `archived` tombstone list before allowing snapshot ingest.

**Verdict:** Correct. The archive guard is properly placed. Test at candidate line 939 confirms this: ingest after archive is blocked.

### 7. Upstream title fallback pattern (session?.title ?? tab.title ?? "Untitled session")
**Candidate claims:** Upstream `packages/tui/src/component/session-tabs.tsx:752,1583` uses `session?.title ?? tab.title ?? "Untitled session"`; context/session-tabs.tsx:118-135 wires live/persisted; `util/session-title-fallback.ts` has timestamped fallback.

**Canonical source confirmation (from `/Users/mymac/Projects/tmp/opencode` pin 450e7b9):**
- `packages/tui/src/context/session-tabs.tsx:119-122`:
  ```rust
  const title = (sessionID: string, persisted?: string, fallback?: string) => {
      const session = data.session.get(sessionID)
      return session?.title ?? persisted ?? fallback ?? (session ? withTimestampedFallback(session) : undefined)
  }
  ```
  Confirmed: session title takes precedence over persisted/tab title, with timestamped fallback.
- `packages/tui/src/component/session-tabs.tsx:752`:
  ```rust
  const title = () => (props.controller ? undefined : session()?.title) ?? tab.title ?? "Untitled session"
  ```
  Confirmed.
- `packages/util/src/session-title-fallback.ts:14-16`:
  ```rust
  export function withTimestampedFallback(info: Info) {
      return info.title ?? fallback(info)
  }
  ```
  Confirmed: `info.title` (daemon rename title) takes precedence over generated fallback.

**Verdict:** The candidate's upstream citation is accurate. The Rust `native_tabs.rs:90-100` `TabInput::from_tab` reads `nav.title(&tab.session).unwrap_or(&tab.session.0)` - meaning the snapshot title (via `nav.title`) feeds into the tab label. This matches the upstream pattern: daemon rename title > snapshot title > raw session ID. The "Untitled session" string fallback is intentionally NOT applied here (canonical `native_tabs.rs` uses session ID as fallback, consistent with its "Never invents session names" rule at line 449).

### 8. Render path wiring (tui_entry.rs)
**Candidate's wiring.diff claims:**
- `tui_entry.rs:3151-3157` startup: `navigation.open(snapshot.session_id)` for the bound session, no title ingest.
- `tui_entry.rs:4523` render: `TabStrip::from_navigation(&navigation, &[])`.
- `native_view_sessions.rs:505` `parse_rows` with `SessionRow { id, title }`, `MAX_TITLE_BYTES=512`.

**Canonical source confirmation:**
- `tui_entry.rs:3137-3139` (canonical e15b...):
  ```rust
  let mut navigation = crate::native_navigation::NavigationState::new();
  if let Some(snapshot) = live.as_ref() {
      navigation.open(crate::native_navigation::SessionRef(snapshot.session_id.clone()));
  }
  ```
  Confirmed: opens session, no title ingest at startup. NOTE: candidate wiring.diff says lines 3151-3157, canonical is 3137-3139. Minor line offset - acceptable (candidate file may differ slightly).
- `tui_entry.rs:4523` (canonical e15b...):
  ```rust
  let tab_inputs = TabStrip::from_navigation(&navigation, &[]);
  ```
  Confirmed exact match.
- `native_view_sessions.rs:505` (canonical 7bbe8d91...):
  ```rust
  pub fn parse_rows(value: &serde_json::Value) -> Result<Vec<SessionRow>, String> {
  ```
  Confirmed. `SessionRow { id, title }` at lines 32-35. `MAX_TITLE_BYTES = 512` at line 28.

**Verdict:** Wiring diff accurately describes the canonical integration points. The candidate does NOT modify `tui_entry.rs` (write scope is `native_navigation.rs` only). The wiring.diff is a handoff note for the runtime owner.

### 9. Snapshot filter to open-tab roster
**Candidate claims:** Runtime owner filters snapshot rows to `navigation.tabs()` open tab roster before ingestion.

**Canonical source confirmation:**
- Canonical `tui_entry.rs:3467-3472`:
  ```rust
  if let Some(snapshot) = live.as_ref() {
      match http_request(&snapshot.origin, "GET", "/api/sessions", None, auth).and_then(|body| {
          serde_json::from_str::<serde_json::Value>(&body).map_err(|error| error.to_string())
      }) {
          Ok(value) => match crate::native_view_sessions::parse_rows(&value) {
              Ok(rows) => view.set_session_rows(rows, Some(snapshot.session_id.clone())),
  ```
  The `GET /api/sessions` call and `parse_rows` are confirmed. The filtering to open-tab roster is a runtime-owner concern (wiring.diff item 1), which the candidate explicitly defers with "NOT applied by this worker."

**Verdict:** Correct scope boundary. The candidate only provides the API; the runtime owner wires the filter. The session listing already exists at `tui_entry.rs:3469` in canonical source.

### 10. Test assertions
Candidate scratchpad claims tests assert:
1. Distinct opaque IDs get distinct snapshot titles
2. Empty preserved `Some("")` vs missing `None`
3. Revisioned rename overrides matching snapshot
4. Stale rev ignored
5. Archive removes snapshot and blocks resurrection
6. No ingestion for non-open session (runtime owner filter)

**Canonical source confirmation:**
- Candidate test `snapshot_titles_project_distinct_sessions_and_preserve_empty` (lines 897-908): confirmed - tests distinct sessions and `Some("")` preservation.
- Candidate test `revisioned_rename_overrides_matching_snapshot_and_stale_is_ignored` (lines 910-929): confirmed - tests rename override and stale rev.
- Candidate test `archive_removes_snapshot_and_blocks_resurrection` (lines 932-949): confirmed - tests archive removes snapshot and blocks resurrection.
- Test 6 (runtime filter) is NOT in candidate's `native_navigation.rs` tests - it's a wiring.diff concern for the runtime owner. Confirmed correct scope.

**Verdict:** All test assertions are present in the candidate file and match the claimed behavior.

## Gap diagnosis

**Gap:** The candidate does not wire the snapshot title ingestion into `tui_entry.rs`. The `wiring.diff` explicitly states this is a runtime-owner concern ("NOT applied by this worker"). Canonical `tui_entry.rs:3137-3139` opens the session but does NOT call `ingest_snapshot_title`. Canonical `tui_entry.rs:3467-3472` fetches `/api/sessions` and parses rows but passes them to `view.set_session_rows`, NOT to `navigation.ingest_snapshot_titles`.

**This is not a defect in the candidate's scope** - it is a deliberate boundary. The candidate writes only `native_navigation.rs` and leaves `tui_entry.rs` wiring unchanged. The runtime owner must apply the wiring.diff to complete the integration.

## Verdict summary

| Criterion | Status | Evidence |
|-----------|--------|----------|
| Bounded snapshot field (MAX_SNAPSHOT_TITLES = MAX_TABS) | PASS | Candidate native_navigation.rs:30-31, 299 |
| ingest_snapshot_title guards archived sessions | PASS | Candidate native_navigation.rs:619-630 |
| ingest_snapshot_titles bulk API | PASS | Candidate native_navigation.rs:634-641 |
| title() precedence: revisioned > snapshot, preserves Some("") vs None | PASS | Candidate native_navigation.rs:647-658, test:907 |
| apply(Renamed) supersedes matching snapshot | PASS | Candidate native_navigation.rs:561-562, 568-569 |
| apply(Archived) removes snapshot | PASS | Candidate native_navigation.rs:580 |
| No fabricated daemon rename events | PASS | No ServerEvent rename channel invented; wiring.diff:26 |
| No session resurrection via snapshot | PASS | Candidate native_navigation.rs:620-622, test:939 |
| Upstream title fallback pattern matches | PASS | session-tabs.tsx:752,1583; session-tabs.tsx:121 |
| Canonical render path wiring verified | PASS | tui_entry.rs:3137-3139, 4523; native_tabs.rs:90-92 |

**APPROVED.** The candidate correctly adds bounded snapshot title ingestion to `native_navigation.rs` with proper precedence, archive guards, and revision supersession. No lifecycle rewrite, no fabricated events, no resurrection. The wiring into `tui_entry.rs` is deliberately deferred to the runtime owner per the candidate's stated scope.

## Remaining work for full integration

The runtime owner must apply `wiring.diff`:
1. Filter `parse_rows` output to open-tab roster: `navigation.ingest_snapshot_titles(rows.filter(open) ...)`
2. At startup after `navigation.open(snapshot.session_id)`: `navigation.ingest_snapshot_title(SessionRef(snapshot.session_id), snapshot.title)`
3. The `snapshot.title` originates from `GET /api/sessions` (snapshot_from_listing), not a rename event, so no revision is fabricated.

## Artifact hashes

- Candidate `native_navigation.rs`: `9b5498335cad7c45e2d540dd8864ec4e5f13ec55bdaa84ed55b3a373f12f9f48`
- Candidate `scratchpad.md`: `407c333ffd1917f59b743452d6d2106b7498f4128b35beca81830d232b7d9f94`
- Candidate `wiring.diff`: `407e121cc1d1ab53512d4f4d0ad7b95120902e9f0c89848832133e94edaf61bd`
- Canonical `native_navigation.rs`: `e15b0334aefb4b7090ba