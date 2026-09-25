# TUI-011 GNU objdump administrative handback

Status: scoped shell GREEN; parent `TUI-011` remains BLOCKED. This handback owns
only this scratchpad and the `TUI-011` ledger row. Product commit is preserved:
`8a89a6fe2e40191b8716415669ca2a942e7c044a` on
`lane/TUI-011-GNU-OBJDUMP-GREEN`.

## Claim and transfer

- Current session: `ses_f26d9154effeEuQu3EDtXLYi1d`.
- Prior implementer: `ses_f2705567affenLQq1gC05YNgKx`, stopped after the step limit.
- Independent verifier: `ses_f26df5cb7ffe4DLtMB6uIpfoVZ`, accepted scoped GREEN only.
- User authorized transfer. `cc.reclaim` then `cc.claim` ran with recorded evidence
  before this administrative edit.
- Owned admin paths: `worklog/TUI-011-GNU-OBJDUMP-RED.md` and the `TUI-011`
  row in `tasks/completion/claims.json`. No product, test, controller, policy,
  or verifier file was edited.

## Frozen contract and scoped result

Frozen test `crates/opentui-bridge/tests/gnu_objdump_wrapper.sh` remains byte
identical at SHA-256
`0cc44d34f45fac563d871fafcedf41a2623599147d93a8cf325f979e642eaa3c`.

The product change at `crates/opentui-bridge/native/build_opentui.sh:403-411`
extracts the architecture token before GNU's comma-separated flags and accepts
GNU `i386:x86-64` plus LLVM `x86_64` for x86_64, while retaining fail-closed
target mapping and existing ELF, ABI, dependency, path, size, and hash checks.

Reported scoped evidence:

- Frozen wrapper test GREEN on the macOS wrapper path; tracked artifact SHA
  `9f074adf1e3c67bb027433d44da05b9e285a37ae58cf0b2ed76504304450c79e`.
- Local aarch64 verify-only GREEN; artifact SHA
  `e85a45710e9e181b3eb7cca877a1d9022f2210bfa1e06b7c159e734506da3b89`.
- Synthetic architecture mapping checks, `sh -n`, and `git diff --check` GREEN.
- `python3 tools/lane_gate.py --run` reported 8 module lanes and 4 test lanes
  GREEN, but this gate is unrelated to the shell test and is not shell-test
  evidence.
- Implementer reported native Ubuntu x86_64 GREEN from Nomad allocation
  `589731a3`, Ubuntu 26.04.1, GNU objdump 2.46, log SHA-256
  `821fe4ddd25227fb510ed0206048a87ce876f752e295d2529e6678f2f2f69ae5`.
  The raw log was not independently inspected here; this is reported evidence.

## Parent obligations and blockers

Parent `TUI-011` is not accepted. Remaining obligations: complete the pinned
native distribution matrix, including MSVC/Windows build and signing; prove a
clean target resolves released native libraries without developer toolchains;
review the frozen manifest plus SBOM, licenses, checksums, and font provenance;
and replay ABI/Unicode/rendering fixtures for the pinned-fork upgrade process on
the integrated parent revision. Independent parent verification and acceptance
remain required.

`python3 tools/validate_repository.py` remains blocked by pre-existing 51
backlog/ownership errors. `python3 tools/convergence_gate.py` remains blocked by
pre-existing 94 off-plan ledger findings. Neither was changed or bypassed.

No Cargo, Nomad, or new test command was run during this administrative transfer.
