# FIX-05 scratchpad: route_state_full.rs

Claim: lane FIX-05 owned file crates/opentui-bridge/src/route_state_full.rs.
Source evidence:
- route_session.rs:12 RouteView Chat/Timeline/Fork, :32 SessionRoute struct.
- page_router.rs:9 PageRouter holder.
- permission_kinds.rs:28 all() 12 kinds.
- question_state.rs:16 QuestionState.
- This file consolidates Home/Session/Plugin only; permission/question/revert/kv unwired (native page state).
Target boundary: Route2 {Home, Session(String), Plugin(String)} + open_session/open_plugin + is_home + id_of; MAX_ID 128 char-safe trunc; empty -> Home.
Tests: 4 tests (open_session_basic, empty_goes_home incl plugin, truncates_to_128 both variants, id_of_and_is_home).
Decisions: fixed open_plugin("") -> Plugin("") gap to Home fail-closed symmetric with open_session; kept trunc chars().take(MAX_ID); rewrote doc comments shorter to fit <=90 lines.
Remaining: none. rustfmt --check EXIT 0.
