# BRIDGE-PAR-184 perm_gate scratchpad

claim: ses_par184, status in-progress -> completed on rustfmt PASS.
source: crates/opentui-bridge/src/permission_ctx.rs:6 PermissionCtx (request/grant/deny/is_granted), permission_full.rs:53 summary pattern.
target: crates/opentui-bridge/src/perm_gate.rs only. lib.rs/Cargo.toml untouched.
tests: 6 in-file (ask queues, allow grants, deny rejects, missing false, invalid false, cap 128).
decisions: ask checks is_granted first, queues via request, always false unless granted; ponytail: no dup-grant guard beyond ctx.
unknowns: none.
