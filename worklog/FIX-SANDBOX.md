# FIX-SANDBOX — closed as covered-by-external (honest boundary recorded)

- DISC-106 (b688c56, verifier session) landed crates/security/src/os_backend.rs (614L):
  real Landlock DETECTION, path confinement via run_confined, inherited
  fds/capability close before exec; 5/5 green + 148/148 lib no-regress.
- Remaining gap (honest, documented in-module): raw Landlock ruleset ATTACH is not
  implemented because the crate is #![forbid(unsafe_code)] with no syscall deps.
  Adding it requires lifting that policy (verifier authority, repo contract) or a
  separate enforcement crate. Not delegated to avoid a policy violation.
- R2-08 P0 status: partially resolved (confinement + capability close + honest
  doctor), raw ruleset attach pending authority decision.
