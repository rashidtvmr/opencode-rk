# APP-009-svc (service_commands.rs)

claim: service control types live, pure-state, real behavior wired.
source: HEAD 5af7884; owned crates/cli/src/service_commands.rs (new, 526 lines);
  sibling evidence crates/cli/src/terminal_host.rs:1-40 (RAII/bounded-queue pattern followed).
scenario: start/submit/stop per policy; detach one of two clients; 50x start/stop cycles.
boundary: owned file only. No other edits. std only, #![forbid(unsafe_code)].
  shutdown.rs (signals/terminal restore) NOT owned, untouched.
tests (frozen in-file, RED todo!() x9 fail -> GREEN 9 pass):
  service_action_roundtrip; stop_drain_completes_pending; stop_cancel_drops_pending;
  stop_drain_respects_bound; submit_rejected_when_stopped_or_full;
  close_one_of_two_preserves_daemon_and_session;
  close_last_client_keeps_daemon_owned_session;
  leak_helper_reports_growth; repeated_start_stop_cycles_no_leak.
decisions: Drain{max_items} overflow reported as cancelled, order preserved;
  detach never touches daemon/session flags; ledger release saturating (no underflow panic);
  ponytail: no wall-clock drain timeout, add when stuck-work SLO exists.
unknowns: none in slice. Signal/suspend/restore covered by shutdown.rs lane.
