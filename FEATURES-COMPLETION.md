# Application completion backlog

This mandatory addendum preserves the entire legacy `ralph.json` and requirements.
It adds **90 vertical parent slices with 450 concrete test scenarios**. These are
specifications, NOT 450 implemented/passing tests and NOT a completion certificate.
Every new slice starts `not-started`. Existing accepted flags need fresh evidence.

| Family | Slices | Scope |
|---|---:|---|
| AUD-001..020 | 20 | Independent source/runtime audits, all legacy IDs, extra requirements and current-dev delta |
| APP-001..012 | 12 | One-command installed app, secure daemon, shared execution, onboarding, history/events and golden journey |
| TUI-001..011 | 11 | Owned Zig ABI/Rust bridge, real renderer, input/composer/transcript/tabs/approvals/settings and native packaging |
| PAR-001..010 | 10 | A-to-Z upstream parity plus providers/routing/agents/security/plugins/tools/web/protocol extras |
| NET-001..015 | 15 | Owned identity/control plane, explicit pairing, scoped protocol, named tunnel, outbound PC connector, files/PTY, replay/revoke and operations |
| MOB-001..006 | 6 | Real iOS/Android installs, login, secure tokens, multi-PC session tabs, control, lifecycle and device acceptance |
| COORD-001..008 | 8 | Actual native adapter, rolling 20-worker scheduling, one-file children, frozen tests, integration, durable leases and budgets |
| SHIP-001..008 | 8 | Platform artifacts, clean-machine/provider/source/remote/device/resource evidence and independent release decision |

Canonical addendum: `ralph.completion.json`. Its five included JSON files contain
each product journey, explicit dependencies, proposed ownership paths and five
concrete acceptance/test scenarios. Audit cards expand the shared audit contract
and exact shard selectors. Use `python3 tools/completion_plan.py --card <ID>` to
read the complete card and inherited safety/TDD contract. No matching selector
or file name is proof that a feature exists.

`python3 tools/completion_plan.py --export` emits both old/new scopes;
`--audit-legacy` assigns every old ID, including unknown prefixes, to an audit.
`--ready` lists initial audit roots only. `--check` validates the union without
trusting accepted flags. `--release --evidence-root <trusted-directory>` rejects
missing/stale proof on the current commit. The old flat export is not authority.

Read `prompts/COMPLETE_APP.md` for delegation. A parent can span several layers,
but each actual worker gets ONE owned file; shared contracts are prewired by the
integrator. Unknown behaviors create mandatory children, not silent exclusions.

## Current evidence boundary

Source inspection established specific launch/TUI/tool/controller gaps in
`docs/audits/2026-09-17-app-completion.md`. The complete upstream repositories have
not been exhaustively reviewed here. The Python specification/scheduler tests
exercise their real helper code with synthetic legacy data and an external-harness
test adapter. They do not prove Rust, OpenTUI, model-provider, OS-sandbox, Cloudflare
or phone-product completion. No native subagent adapter is silently installed.
