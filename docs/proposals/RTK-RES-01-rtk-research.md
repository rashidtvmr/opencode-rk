# RTK-RES-01: Rust Token Killer filter-behavior research

Status: proposal research only. No product code.
Sources: AGENTS.md agent-operating-rules RTK section; tasks/TOOL-014.md (bounded OutputStore); docs/SECURITY.md section 6 (never disable safeguards, stop on failure); docs/proposals/RTK-SLICE-01-core-spec.md; local probes against rtk 0.43.0 (`~/.local/bin/rtk`); GitHub issue-search listing for rtk-ai/rtk (titles + snippets only, bodies truncated by fetch proxy, noted per issue).

## 1. Filter-category table (observed `rtk --help`, v0.43.0)

| Category | Subcommands | Claimed shrink |
|---|---|---|
| git | `git` | compact status/diff/log |
| ls/read/grep | `ls`, `tree`, `read`, `find`, `diff`, `grep`, `rg` | token-optimized listing, truncate (grep: max-len 80, max 200), group by file |
| test | `test`, `jest`, `vitest`, `playwright`, `cargo`, `dotnet` | failures only |
| build/lint | `tsc`, `next`, `lint`, `prettier`, `format`, `npx` | grouped errors, strip boilerplate/ASCII art |
| analysis | `err`, `log`, `json`, `summary`, `smart`, `deps`, `env`, `wc`, `curl`, `wget` | errors/warnings only, dedup, heuristic summary |
| gh | `gh`, `glab` | token-optimized |
| docker/pkg | `docker`, `kubectl`, `oc`, `aws`, `psql`, `pnpm`, `npm`, `prisma` | compress tables/JSON, force JSON |
| passthrough | unknown/no-filter command, bare command without `rtk` | byte-identical per AGENTS.md ("always safe") |

## 2. Issue list (symptom + root cause + design fix)

I-01 `rtk err` masks exit code. Probe: `rtk err ls /nonexistent-xyz` prints `[FAIL] ... exit code: 2` then shell reports `exit=0`. Symptom: caller branching on `$?` takes success path on failure. Cause: wrapper exits 0 after rendering its own report. Fix: `std::process::exit(child_code)` as last act; render report on stderr, never convert failure to 0.

I-02 `rtk proxy` masks exit code and misparses chains. Probe: `rtk proxy 'echo OUT; echo ERR >&2; exit 3'` echoed the literal string, `exit=0`. Symptom: compound commands never execute, failures invisible. Cause: args forwarded without shell, exit unpropagated. Fix: document proxy as single-argv passthrough only; propagate code; AGENTS.md already mandates prefixing each chain segment, keep that rule.

I-03 golangci-lint exit 1 mapped to 0 plus "No issues found". GitHub search snippet, rtk-ai/rtk#3870 (body truncated on fetch, title + snippet verified): `Ok(if exit_code == 1 { 0 } ...)` while stderr dropped, hard errors reported as clean. Symptom: lint gate passes on dirty tree. Cause: tool-specific success-code assumption baked into generic wrapper. Fix: never remap codes in the filter layer; per-tool code tables live in caller config, default identity.

I-04 prettier failures hidden by stdout-only capture. Search snippet, rtk-ai/rtk#2878: prettier 3 writes `[warn]` lines and the `Code style issues found` summary to stderr, `stdout_only()` capture never sees them. Symptom: unformatted tree reported clean. Cause: capturing one stream. Fix: always capture both streams; stderr preserved verbatim (RTK-SLICE-01 failure-states rule); budget applies with its own marker, never drop.

I-05 spawn-failure message swallowed when tool missing. Search snippet, rtk-ai/rtk#3026 (`uv run rtk <tool>`): exit nonzero correct but stdout single newline, stderr empty. Symptom: agent retries blind, cannot distinguish missing binary from empty result. Cause: spawn error path emits no diagnostic into either stream. Fix: spawn errors synthesize `Filtered`-style record with `code=127`, message on stderr, `truncated=false`.

I-06 pnpm exit code not propagated. Search snippet, rtk-ai/rtk#2658 (`fix(pnpm): propagate exit code from pnpm outdated`). Symptom: outdated/vulnerable deps look current. Cause: same class as I-01, per-subcommand. Fix: shared exit-propagation choke point in native library so no subcommand can reintroduce it (covers I-01/I-03/I-06 at once).

I-07 grep exit-code inversions + hidden-dir blindness. Search snippet, rtk-ai/rtk#3432: binary-file match becomes 1 (not found), missing root becomes 0 (success), hidden dirs invisible to `rtk find`. Symptom: `if rtk grep -q AKIA .` takes wrong branch; secrets in `.bin`/NUL-containing files reported absent. Cause: filter rewrites grep's 0/1/2 contract and prunes dotfiles silently. Fix: byte-preserve grep/find exit codes (0 match, 1 no-match, 2 error); any path pruning is opt-in, logged in marker.

I-08 grep return-code conflation, independent corroboration. Search snippet, adhityaravi/maki#710: `search_text` discards `proc.returncode` and stderr, returns "No matches" on empty stdout, so invalid regex and permission-denied are indistinguishable from no matches. Symptom: broken pattern debugged as absent code. Cause: same antipattern, different codebase. Fix: map 2/error to error record with stderr attached; 1 to explicit no-match record; never merge.

I-09 ANSI codes leak through `rtk grep`. Probe: `printf '\033[31mred\033[0m\n' | rtk grep red | cat -v` retains `^[[31m...^[[0m`. Symptom: color bytes consume token budget the filter claims to save; downstream parsers choke. Cause: no CSI strip before filter. Fix: strip ANSI CSI first, then shrink (RTK-SLICE-01 observable-contract order).

I-10 `rtk read` passes 5000-char single line untruncated; chained `rtk` segments drop output. Probes: 5000x`x` line returned whole; repo note `docs/upcoming-features/claude-code-harness.md:75` records "grep returned empty due to rtk chaining". Symptom: huge-line blowup defeats byte budget; piped filters yield empty results. Cause: line-count truncation without char budget; chained wrappers re-filter already-filtered bytes. Fix: byte budget with `char_boundary` split + marker (TOOL-014 precedent); pipe contract: filter idempotent, second application is passthrough.

Heuristic-summary nondeterminism (design risk, no issue access): `smart`/`summary` advertised "heuristic-based" in `--help`; web search backend returned no results so no external complaint citable. Fix: deterministic filters only in hot path; heuristics behind explicit flag, output labeled, never default.

## 3. Native-inbuilt architecture recommendation

- Rust library, not external binary: `crates/rtk_core` pure function `filter(kind, stdout, stderr, code, cfg) -> Filtered` per RTK-SLICE-01; no `Command` spawn, no thread, no I/O, O(n) time, O(max_bytes) memory. Kills I-02/I-05 spawn class and per-agent-process rule (PLAN.md ADR-001).
- Raw-output escape hatch: `raw()` returns byte-identical streams and code; AGENTS.md bare-command rule becomes a typed API. Debug path always available, satisfying SECURITY.md section 6 (stop, preserve reproduction, never disable safeguard to keep loop moving).
- Exit-code preservation: filter never alters `code`; stderr verbatim; ANSI strip first; over-budget stdout truncated at `char_boundary` with `TRUNC_MARKER`; empty input passthrough with `truncated=false`.
- One shared exit-propagation choke point; per-tool code tables forbidden in filter layer (fixes I-01/I-03/I-06 structurally).

## Sources / honesty note

- Verified local: rtk 0.43.0 `--help`, I-01/I-02/I-09/I-10 probes above, AGENTS.md:86-90, TOOL-014 contract, SECURITY.md:75-82, RTK-SLICE-01.
- Issue titles/snippets I-03..I-08 from GitHub issue-search page fetch; full bodies truncated by proxy, no URLs/numbers invented beyond what the listing showed. Websearch backend returned no results for all queries; no other external complaints citable.
