# Native configured commands

Define reusable prompts in the native V2 `commands` object of `opencode.json`
or `opencode.jsonc`:

```json
{
  "commands": {
    "review": {
      "description": "Review a package",
      "template": "Review $ARGUMENTS for bugs.",
      "agent": "build",
      "model": "openai/gpt-5.6"
    }
  }
}
```

Enter `/review src/auth` in the chat composer. `/commands` and Ctrl-K show
configured entries alongside local commands. Selecting an entry prefixes the
current draft with that command; Enter submits it. The original draft remains
available if validation or admission fails. Local navigation commands such as
`/models`, `/connect` and `/appearance` retain precedence.

Markdown definitions are also discovered in `.opencode/commands/**/*.md` and
the global OpenCode `commands` directory. For example,
`.opencode/commands/team/review.md` defines `/team/review`:

```markdown
---
description: Review a package
agent: build
---
Review $ARGUMENTS for bugs.
```

Legacy singular `command` directories and configuration objects are accepted.
Global configuration is loaded first, followed by ancestor/project documents;
`.opencode` documents override direct documents. Changes are loaded when listing
or executing commands. Markdown frontmatter supports scalar `description`,
`agent`, `model`, `variant`, `subtask` and `subagent` fields.

`$ARGUMENTS` preserves the complete argument tail, including newlines. `$1`, `$2`
and higher positions parse quoted arguments; the highest numbered placeholder
consumes the remaining arguments. Without placeholders, nonempty arguments are
appended after a blank line. The expanded prompt is durably stored and rendered
through the same streaming turn and permission broker as normal prompts.

The command model overrides the selected agent's model, which overrides the
composer model. A model `#variant` suffix or explicit `variant` selects reasoning
effort. Configured primary agents can supply a prompt and model. `plan` exposes
only read, grep and glob tools. Disabled/unknown agents are rejected.

Background/child-session commands, agent-specific permission overrides, advanced
YAML frontmatter and shell interpolation inside templates are currently rejected
with an error. For an interactive shell command, use the brokered native
`! pwd` mode. Rejection never silently runs a template as an ordinary prompt.
