# BRIDGE-048 worklog

Claim: context plumbing (ReadyGate, frozen RuntimePaths, base-relative format).
Evidence: TS checkout a0d9b6c.
- helper.tsx:15 `Show when={init.ready === undefined || init.ready === true}` -> ReadyGate check/wait; default ready.
- runtime.tsx:25-32 provider `Object.freeze({ ...value })` -> RuntimePaths private fields, getters only.
- runtime.tsx:46-50 `required` throws when missing -> `new` errs on empty field.
- path-format.tsx:15-24 `formatPath` in-base -> relative, else `abbreviateHome` -> `format_in_base` via `context_kv::Project::display` with empty home (absolute fallback); tildefy reused, not redefined.
- abbreviateHome runtime.tsx:3-9 consulted; `Project::display` already covers it.
Tests: 7 (gate pass/block, paths reject/getters, format relative/dot-empty/outside-absolute). No cargo run (scope ban); logically green.
Unknowns: none.
