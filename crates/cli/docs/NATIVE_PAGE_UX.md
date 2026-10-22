# Native page presentation

Native `oc2` uses a shared visual hierarchy across its pages and overlays:

- Page panels have outer margins, inner padding and a readable maximum width.
- Headings and section titles use the theme accent and bold weight.
- Item labels use normal text; descriptions and metadata use muted text.
- The focused item has a full-width accent background and contrasting bold text.
- Search/results and results/footer have breathing room on roomy terminals.
- Compact layouts reduce whitespace while keeping the selected item in view.

This applies to Sessions, Help, Models, Providers, Agents, Workspaces, Theme,
Status, MCP, Diff, Context and Commands. The command palette, provider/model
setup, permission/question/confirmation/error dialogs, subagent roster, sidebar,
appearance editor and composer recommendations use the same hierarchy.

Use **Up/Down** to move through selection lists. Navigation does not activate an
item; **Enter** confirms it. Permission dialogs still begin without a selected
decision and require explicit confirmation. In the appearance editor,
**Up/Down** or **Tab** changes the highlighted box target; editing keys retain
their existing meanings.

Styles are separate from display strings and survive ANSI sanitization. Page
renderers supply semantic row roles and explicit bounds to the native painter,
so headings and selection are not inferred from arbitrary transcript text.
Appearance preferences, per-box overrides and saved themes remain authoritative.

## Daemon discovery

The interactive client discovers its service using the authenticated descriptor
under its own data directory. A new service binds an available loopback port;
it does not trust another process merely because `/health` answers on port 4096.
`OPENCODE_RK_DAEMON_ADDR` can still explicitly choose a listen address. Reopening
the client reuses the descriptor's service and existing session data.
