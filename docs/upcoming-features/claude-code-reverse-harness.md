# Scratchpad: claude-code-reverse harness research

## Claim
The claude-code-reverse repo is a runtime-tap observability harness, not a protocol spec. Transferable value to opencode-rk: provider-boundary tap format, log parser with prompt/tool dedupe, static transcript viewer, and agent-workflow patterns (compaction, subagent isolation, cheap-model chores).

## Source evidence
- Reverse repo HEAD: `c0d99ea1ab7168c12ba74838cfea355ce10f6c56` via `rtk git rev-parse HEAD` in /home/rashid/projects/tmpcodes/claude-code-reverse.
- opencode-rk HEAD: `a754746dbaeb136286ef1862017ae49e68e89289` via same in /home/rashid/projects/opencode-rk.
- Method statement: `README.md:20-26` (ignore internals, tap LLM API boundary); patch target `README.md:46-48` (`beta.messages.create`); viewer/pipeline `README.md:59-64` (parser.js + visualize.html, frequency/position heuristic for common prompts).
- Tap implementation: `cli.js.patch:145-176` (wrap `beta.messages.create`, per-start log file, `uid`, `input`/`output`/`error` lines, non-stream vs `_thenUnwrap` stream path); stream tool-use accounting `cli.js.patch:26-138` (`tapIteratorInPlaceWithTools`, content_block start/delta/stop, input_json deltas, byte counts, durationMs).
- Log line protocol: `parser.js:163-231` (Pattern A `ISO uid=<uid> kind:` with kind in input/output/stream.final/error; Pattern B `request_id`; first non-empty line = session title; warn-and-skip on unrecognized/JSON-fail).
- Dedupe transform: `parser.js:40-82` (tool registry keyed by stable-stringify, `Name_N` ids; prompt registry `prompt_N` with refcounts); `parser.js:86-136` (user text passes through unless `<system-reminder>`; system/tools/messages replaced by ids); kind guess `parser.js:233-238` (count>1 = system_like).
- Viewer: `visualize.html:1-120` + total 1061 lines (single static file, file picker, card/detail/pill layout, marked+DOMPurify CDN). No backend.
- Reversed behaviors: `README.md:84-154` (quota check Haiku max_tokens=1 text `quota`; topic check Haiku; core workflow Sonnet + system-reminder start/end; compaction prompts; IDE open-file prompt + MCP tools; TodoWrite JSON at `~/.claude/todos/` reloaded via reminder; Task tool subagent with dirty-context isolation; summarize-previous-conversation Haiku).
- Corpus: `logs/` 8 files (basic, compact, ide-integration, 2x output-style multi-MB, setup, sub-agent, summarize); `results/prompts/` 12 .md; `results/tools/` 15 .yaml; `v1/` archived LLM-on-uglified-JS experiment (split/learn/merge/ask scripts).
- opencode-rk targets: `PLAN.md:61-76` (ADR-001/002 single Tokio daemon, lazy locations); `PLAN.md:87-94` (ADR-004 SQLite + content-addressed blobs, retention per category); `FEATURES.md:9-48` (REQ->story map: SESS/DB/PROV/AGENT/OPS/UI families).

## Observed scenario
Ran read-only inventory only. `logs/summarize.log` head shows quota probe (`claude-3-5-haiku-20241022`, `max_tokens:1`, content `quota`) confirming README quota claim. No product code executed, no patches applied, no network calls. v1 scripts not run (needs API key, archived path).

## Target boundary in opencode-rk
- Provider tap + redaction: PROV family (request/response/error envelope at provider boundary), SEC family (secret scrub before persist).
- Structured transcript store + retention: SESS-001/DB-003 (canonical history), DB-008/DB-009 (blob/GC policy), REQ-033.
- Compaction + summarize: SESS family (context window management), AGENT-011/DB-003 (persist summaries).
- Subagent dirty-context isolation: AGENT-003/004/005/011 (parent keeps final result only).
- Cheap-model chores (quota/topic/title/summarize on Haiku-class): ROUTE/CAT families (model routing, effort levels REQ-018).
- Offline transcript viewer / debug export: UI-006 (status), SESS-017 (history workflows), OPS (diagnostics bundle).

## Tests / verification performed
- `rtk git rev-parse HEAD` both repos: OK, hashes above.
- `rtk ls` inventory: reverse root (README, patch, parser, visualize, v1/, results/, logs/); results/prompts 12 files; results/tools 15 files; v1/scripts 8 files; opencode-rk docs/ + tasks/ listed.
- `rtk proxy wc -l`: parser.js 277, visualize.html 1061, cli.js.patch 179, README.md 154.
- `rtk proxy head` on summarize.log confirmed quota-probe line shape.
- Re-read of this file after writing: pending (do on finalize).
- No Rust tests run (research-only lease; no crates/ edits).

## Decisions
- Recommend tap-at-provider-boundary as the single observability seam; reject SDK monkey-patching as the mechanism (native Rust middleware instead).
- Recommend dedupe-by-canonical-hash for prompts/tools as storage optimization, not as semantic classifier (frequency heuristic is viewer-grade only).
- Recommend subagent context isolation and compaction/summarize as behavior patterns worth specifying; reject copying prompt text verbatim.
- Viewer: recommend offline debug-export artifact over CDN-dependent HTML.
- v1 (LLM over minified JS): no transfer value beyond DISC-phase technique note.

## Remaining unknowns
- Exact current line counts of visualize.html sections (only first 160 lines read; rest is CSS/render JS by structure).
- Full content of the 12 prompts / 15 tool YAMLs (names inventoried, bodies skimmed only via README references).
- Whether opencode-rk PROV tasks already specify a tap/redaction point (ralph.json grep for transcript/observability returned no match; task cards not all read).
- Upstream license constraints on reusing prompt/tool text (not checked).

## Ranked Top 6 feature suggestions

### 1. Provider-boundary tap with redaction (NEW-REQ proposal: PROV-OBS tap)
What: middleware at provider request/response/stream-final/error seam emitting uid-keyed structured records; secrets scrubbed before persist.
Lean value: one seam covers all providers, debugging without reproducing; tiny code, large diagnostic payoff.
Cost/risk: Low-Med. Risk is log volume and secret leakage; bound bytes + redaction list + off-by-default in perf path.
Maps to: NEW-REQ (no transcript/observability match in ralph.json); adjacent PROV-011 (REQ-010), SEC-004/005 (REQ-027), OPS-001 (REQ-001/024/032).

### 2. Content-addressed transcript store with prompt/tool dedupe (DB/SESS)
What: store turns once; repeated system prompts/tool schemas stored by canonical hash, turns reference ids (parser.js:15-27,48-82 pattern in Rust).
Lean value: directly serves REQ-033 embedded storage and large-history memory targets; mechanical, testable.
Cost/risk: Low. Hash+refcount table; risk is over-normalization hurting readability (keep raw-export path).
Maps to: DB-008/DB-009 (REQ-033), SESS-001/DB-003 (REQ-006/014).

### 3. Compaction + session-start summary (SESS/AGENT)
What: explicit compact trigger (manual + budget threshold) producing one summary block as next-session seed; startup rehydration summary of previous session.
Lean value: proven context-saver from reverse corpus (compact.log 659K, summarize.log 67K); small state machine.
Cost/risk: Med. Risk is lossy summary silently dropping obligations; require format schema + acceptance fixtures.
Maps to: SESS family (REQ-006), AGENT-011/DB-003 (REQ-014).

### 4. Subagent dirty-context isolation (AGENT)
What: child gets extracted task as initial prompt; parent persists only final tool result, child trace GC-able.
Lean value: main-context savings + cleaner audit; matches observed Task-tool design (README:140-146).
Cost/risk: Low-Med. Ownership/lifetime rules needed (PLAN ADR-002); risk is losing debug trace (keep bounded child blob).
Maps to: AGENT-003/004 (REQ-009/012), AGENT-005 (REQ-010), AGENT-011 (REQ-014).

### 5. Cheap-model chores: quota/topic/title/summarize routing (ROUTE/CAT)
What: route tiny classifiers (quota probe max_tokens=1, new-topic check, title, summaries) to cheapest capable model, independent of main effort level.
Lean value: cost/latency win with near-zero behavior risk; exercises independent effort settings.
Cost/risk: Low. Risk is misclassification; chores are advisory-only (title) or fail-open (quota).
Maps to: CAT-004/AGENT-006 (REQ-018), ROUTE family (REQ-022), UI-006 status title (REQ-016).

### 6. Offline debug-export bundle + minimal viewer (OPS/UI)
What: `export-debug` producing one redacted JSONL bundle (session meta, deduped defs, turns, timings, errors) plus a dependency-free local renderer (no CDN).
Lean value: makes every bug report reproducible; static renderer is throwaway UI, not product surface.
Cost/risk: Low. Risk is bundle size (cap + truncation markers) and accidental secret inclusion (reuse tap redaction).
Maps to: OPS family (REQ-001/024/032), SESS-017 (REQ-006), UI-006 (REQ-016); NEW-REQ if OPS cards lack diagnostics scope.
