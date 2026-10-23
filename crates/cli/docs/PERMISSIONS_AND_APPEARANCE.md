# Native permissions and appearance

## Tool decisions

Permission prompts offer six explicit decisions:

1. **Allow once** — approve only this operation.
2. **Deny once** — refuse only this operation.
3. **Allow for this session** — remember the matching operation for this session.
4. **Always allow** — save the matching operation decision.
5. **Deny for this session** — refuse matching operations in this session.
6. **Always deny** — save a matching-operation denial.

Use **Up/Down** or **1–6** to choose, then **Enter** to confirm. With a mouse,
click a choice and then the **confirm footer**. Clicking outside the dialog
does nothing. Escape denies this request without saving a rule. Nothing is
preselected; a decision is not dismissed until the daemon acknowledges it.

`/permissions` (alias `/permission`) opens the per-tool rule editor. Select a
tool with arrows or mouse, choose **Allow**, **Ask**, or **Deny** with **1/2/3**
or the matching buttons, and use **Tab** or the scope row to switch between
session and saved rules. **Ask** removes the selected remembered rule.
Mandatory protections still apply to allowed tools.

The native terminal interface uses labelled numbered choices, visible focus,
contrasting selection, explicit confirmation, keyboard and mouse operation.
These are accessibility improvements, not a claim of web WCAG/WAI-ARIA AA
certification: terminal screen-reader support depends on the terminal itself.

## Appearance

`/appearance` starts on the composer rather than all components. Pick an
individual component using arrows, Tab, or the component list. Choose sharp,
rounded or soft terminal corners and padding independently. Click the option
or use its keyboard shortcut. Reset affects the selected component only.

Corner changes preserve surface identity: a normal assistant response stays
unbordered, the user prompt retains its own treatment, and the composer retains
its input surface. Explicit border-weight preferences remain separate.

Terminal cells support discrete corner glyphs, not continuous pixel curves.
Rounded/soft settings therefore approximate curves rather than claiming a
continuously changing 0–50 pixel radius.

## Importing existing configuration

On first launch, the candidate offers to discover supported OpenCode, Codex and
Claude Code configuration. Declining remembers that the offer was shown.
Use **Commands → Import settings** or `/import` to open it again.

Discovery and preview do not apply configuration. Review the sources, supported
values, conflicts and unsupported-field diagnostics, then explicitly choose
**Apply**. Existing destination values win by default. Source files are left
unchanged; imported commands are not executed, and imported permissions do not
grant authority to the runtime permission broker.

The destination is `opencode.json` under `OPENCODE_RK_CONFIG_DIR`, or under
`$XDG_CONFIG_HOME/opencode-rk` when that variable is set, otherwise
`$HOME/.config/opencode-rk`. Provider and model identifiers retain their exact
identity. Supported credential environment references remain references; raw
credential values are not copied into preview output. Unsupported settings are
reported rather than silently treated as working configuration.

An explicit recent model selection takes precedence over an imported default.
Without a recent selection, a valid imported provider/model is discovered at
startup and can drive the real request. These flows have focused native
candidate verification; exact-release installed verification is still pending.

## Context sidebar

The Context section displays provider-reported input/output, cached and reasoning
tokens after a turn, and model capacity when the catalogue supplies it. Cached
and reasoning tokens are subsets, not additional context consumption. Unknown
usage/capacity is labelled as unreported rather than fabricated. `/sidebar`
opens the panel on compact terminals.
