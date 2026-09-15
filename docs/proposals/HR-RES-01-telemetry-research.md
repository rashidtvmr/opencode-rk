# HR-RES-01: Headroom-style session telemetry research

Status: research only. No product code. No network egress.
Sources: PLAN.md sections 1, 9; docs/SECURITY.md sections 2, 6; tasks/REL-004.md; config/resource-targets.json; Headroom `/stats`, `/stats-history`, `/metrics`, SDK session stats.

Guarantee: telemetry never leaves the machine. It never logs secrets, keys, prompts, completions, or tool message bodies.

## 1. Metric inventory (headroom-style)

| Metric | Headroom precedent | Type |
|---|---|---|
| requests total / cached / rate_limited / failed | `requests`, `headroom_requests_total` | session counter |
| tokens input / output / saved / savings_percent | `tokens`, `headroom_tokens_saved_total` | session counter |
| tokens_before / tokens_after / tokens_saved per compress | `POST /v1/compress` result | per-call numbers |
| compression_ratio (`after/before`, lower better) + avg | result field, `compression_ratio_avg`, histogram bucket | per-call + aggregate |
| transforms_applied / transforms_summary labels | compress result, session `transforms` block | label counts |
| cost total_usd / savings_usd, lifetime compression_savings_usd | `cost`, `persistent_savings.lifetime` | caller-supplied money only |
| cache entries / hits / hit_rate | `cache`, `headroom_cache_hits_total` | session counter |
| latency / overhead / TTFB summary, p99 target <10ms | Prometheus/OTEL summaries | latency histogram |
| per-model and per-provider request counts | Prometheus labels, OTEL counters | label counts |
| per-stage pipeline + waste-signal token totals | pipeline metrics | label counts |
| lifetime tokens_saved + recent_history checkpoints | `persistent_savings`, `proxy_savings.json` | durable total + ring |
| hourly / daily / weekly / monthly rollups | `/stats-history` series | derived rollups |
| session block: requests_total, input_before/after, saved_total, output_total, cache_hits | SDK `get_stats()` | session snapshot |
| config block: mode, provider, enabled transforms | SDK `get_stats().config` | local config echo |

REL-004 precedent (adopt, do not duplicate): input/output/cache-read/cache-create tokens, durations, lines changed, web-search count, unknown-cost flag with sticky unknown state and saturating counters.

## 2. Privacy classification

Safe (aggregate counters only, local-only):
requests, tokens in/out/saved, ratios, transform labels, cache counts, latency buckets, cost totals from caller-supplied micro-USD, rollup checkpoints, config echo (mode/provider/flags), per-model counts without prompt content.

Never-collect:
prompt/completion text, tool args/outputs, full message bodies (`--log-messages` equivalent), secrets/keys/tokens, `.env` content, file bytes, memory-store content, Langfuse-style trace payloads, anything requiring a network beacon or OTLP export. No anonymous beacon. No OTEL export by default.

Per docs/SECURITY.md section 2: no secret logging, no transcript embedding, no broad filesystem access on the telemetry path. Per section 6: never disable this safeguard to keep telemetry working.

## 3. Retention and bounds (per ADR-004)

- Session counters: saturating `u64`, REL-004 style. No wrap. No per-request log.
- Recent history: fixed ring, max 500 returned / 2048 stored checkpoints (headroom `history_summary` precedent). Oldest compacted into rollups, never silently deleting user history to hit a byte target.
- Rollups: four fixed derived series (hourly/daily/weekly/monthly), bounded row counts, recomputed from ring.
- Durable total: one row/file for lifetime totals (headroom `proxy_savings.json` precedent maps to one SQLite row in workspace store, crash-safe write, tolerant of missing/malformed state).
- Disk quota: telemetry tables get explicit byte/row quota under `resource-targets.json` review; `stateless` mode keeps everything in memory with zero filesystem writes.
- Scope: per-session live counters plus one daemon-wide lifetime row. No unbounded queue, no unbounded retained output, every category has quota or archival rule.

## 4. Minimal native design recommendation

- One Rust module (e.g. session telemetry counters), sync `record()` of caller-supplied numbers only. No clock, task, queue, network, or pricing lookup inside.
- Local-only read API mirroring `/stats` shape (live + lifetime + recent preview). No beacon, no OTLP unless explicit separate opt-in outside this slice.
- Opt-out with zero overhead: single `enabled=false` flag checked first; when off, `record()` returns immediately, no allocation, no spawned task, no file write. Default off, matching headroom `--telemetry` off-by-default.
- Reuse REL-004 money handling: caller-supplied integer micro-USD, sticky unknown-cost flag. This slice never fetches prices.
- Whole-tree measurement per PLAN.md section 9: counters cover daemon only; resource acceptance still measures full process tree separately.
