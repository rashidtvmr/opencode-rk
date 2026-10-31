# Matt Pocock Skills — vendored reference

A read-only mirror of [mattpocock/skills](https://github.com/mattpocock/skills)
("Skills For Real Engineers", MIT © 2026 Matt Pocock), vendored here as
**reference documentation**.

These are **not** registered as OpenCode skills. They live under `docs/` and are
not in `.opencode/skills/`, so they are never model-invoked and never appear in
the skill catalog. They are reference material to read, quote, and adapt.

## Provenance

| Field | Value |
| --- | --- |
| Upstream | `https://github.com/mattpocock/skills` |
| Commit | `c55ee46073ed923f86ce59a5eb3b6d895095d1b7` |
| Vendored on | see `.vendored-on` |
| License | MIT, © 2026 Matt Pocock — see `LICENSE` |

`UPSTREAM-README.md` is the author's original README, kept for attribution.
`CONTEXT.md` and `CLAUDE.md` are the author's own domain/context notes.

## Layout

Upstream paths are preserved verbatim, so any upstream link or cross-reference
resolves unchanged.

| Path | Contents |
| --- | --- |
| `skills/engineering/` | 19 skills for code work |
| `skills/productivity/` | 7 general workflow skills |
| `skills/misc/` | 4 niche/tooling skills |
| `skills/in-progress/` | 8 work-in-progress, explicitly unstable upstream |
| `skills/deprecated/` | retired-skill pointer only |
| `docs/` | 25 published write-ups, one per released skill |

Each skill lives at `skills/<category>/<name>/SKILL.md` and may ship supporting
material beside it — `agents/openai.yaml`, extra guides such as `mocking.md` and
`DEEPENING.md`, and in two cases a shell template under `scripts/`.

## Deliberately excluded

| Excluded | Why |
| --- | --- |
| `package.json`, `package-lock.json` | npm plumbing for the upstream repo, 48KB of no reference value |
| `.changeset/` | release automation for the upstream repo |
| `scripts/`, `.github/`, `.claude-plugin/` | upstream CI and publishing machinery |
| `.git/` | this copy is vendored, not a clone |

`skills/*/scripts/` **is** kept: `hitl-loop.template.sh` and
`block-dangerous-git.sh` are referenced by their skills.

## Caveats

- `skills/in-progress/` is unstable by the author's own labelling. Treat it as
  rough notes, not settled practice.
- Several skills assume tooling this repository does not use: an issue tracker
  (GitHub/Linear/local files), `CONTEXT.md` domain docs, ADRs, and TypeScript
  deep-module tooling. Adapt rather than adopt wholesale.
- `/setup-matt-pocock-skills` writes project config and is interactive. Read it
  as reference; do not run it against this repository.
- Do not edit files here. They are a vendored mirror — see refreshing below.

## Refreshing

Re-vendor from upstream and update the provenance table:

```sh
git clone --depth 1 https://github.com/mattpocock/skills.git /tmp/mp-skills
DST=docs/reference/matt-pocock-skills
rm -rf "$DST" && mkdir -p "$DST"
cp -R /tmp/mp-skills/skills /tmp/mp-skills/docs "$DST"/
cp /tmp/mp-skills/{LICENSE,CONTEXT.md,CLAUDE.md,README.md} "$DST"/
mv "$DST/README.md" "$DST/UPSTREAM-README.md"
git -C /tmp/mp-skills rev-parse HEAD > "$DST/.upstream-commit"
date -u +%Y-%m-%d > "$DST/.vendored-on"
```

Then diff against git to review what upstream changed, and update the commit
hash above.

## Related

This repository has its own, authoritative process documentation. Where the two
disagree, the local docs win:

| Local doc | Overrides |
| --- | --- |
| `../../TDD.md` | `skills/engineering/tdd/` |
| `../../CONVERGENCE.md` | `skills/engineering/wayfinder/`, `to-tickets/` |
| `../../SUBAGENT_CONTROL.md` | `skills/engineering/triage/`, `implement/` |
| `../../REPOSITORY_PROTECTION.md` | `skills/engineering/code-review/` |
| `../../../AGENTS.md` | all skills — it is the binding worker contract |
