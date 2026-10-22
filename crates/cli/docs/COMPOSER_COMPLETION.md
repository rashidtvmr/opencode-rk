# Native composer recommendations

In `oc2`, type `/` at the beginning of the draft to browse local and configured
commands. Type `@` at the beginning of a word to browse agents and workspace file
references. Suggestions appear immediately above the composer and filter as you
type.

- **Up/Down** moves the highlighted row.
- **Tab/Enter** inserts the selected reference or command, followed by a space.
  Submit the completed draft with the normal submit key.
- **Escape** dismisses recommendations while retaining the draft. Editing the
  draft allows recommendations to open again.

Credential, permission, command-palette and other dialogs retain input priority.
The popover uses the composer's width and available space above it, with a
scrolling selection on short viewports.

Configured command and agent metadata comes from the authenticated local daemon.
Agents marked hidden, disabled or primary-only are omitted. Workspace file
recommendations are relative names, indexed locally without opening file
contents. The index skips hidden paths, symlinks and common dependency/build
directories; its entry count, depth, retained names and scan time are bounded.
Paths containing whitespace are currently omitted.

Mention completion inserts plain `@agent` or `@relative/path` text into the
existing native turn contract. It does not attach file contents or start a child
turn automatically. Actual file reads and child execution continue through the
existing model tools and permission broker.

Upstream reference: pinned `95daf90670b7c039c436c85537da5fbfe2205b41`,
`packages/tui/src/component/prompt/autocomplete.tsx` (`position`, `filter`,
`insertPart`). The native flat-text turn contract does not yet carry upstream's
structured prompt parts.
