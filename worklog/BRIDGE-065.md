# BRIDGE-065 worklog

Claim: extend sidebar.rs with TS-evidenced rows (a0d9b6c).
RED: new tests reference missing types, fail compile.
GREEN (logical, cargo not run per scope): additive only.

Added (all additive, compat kept):
- ContextUsage{tokens,percent,cost}+display (context.tsx:19-44)
- FileRow.additions/deletions (files.tsx:8; dirty/diagnostics kept, divergence noted)
- LspRow{id/128,root/1024,status}+LspStatus 4v (lsp.tsx:11,27-43; wire connected|error types.gen.ts:2367-2372; Connecting/Off cover transitional/disabled states)
- McpStatus 5v (mcp.tsx:20-27,61-70; types.gen.ts:2380-2407) as McpRow.status:Option (enabled kept, active() falls back); error:Option 512 (tui.ts:442)
- TodoStatus 4v (session-todo.ts:10; todo-item.tsx:16-19) as TodoRow.status:Option (done kept, is_done() falls back)
- SidebarFooter{dir/1024,branch,version} (footer.tsx:19-30,76)
- SidebarRow::Lsp variant

Tests (7 new incl. row_shapes update): context_usage_display, file_row_diff_counts, lsp_bounds_rejected, mcp_status_active_and_error_bound, todo_status_mapping, footer_bound_and_lsp_row_in_state, row_shapes extended.

Unknowns: TS LSP has no connecting/off wire states (display-only); TS footer branch only shown when session dir matches cwd (footer.tsx:23) - not modeled; cost display format ($ fixed 2dp, TS uses Intl USD - equivalent for tests).
