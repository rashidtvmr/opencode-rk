//! Tool abstractions and built-in tool implementations.
#![forbid(unsafe_code)]

pub mod auto_steps;
pub mod builder;
pub mod compat_commands;
pub mod diff_tool;
pub mod elicitation;
pub mod executor;
pub mod ext_commands;
pub mod ext_compat;
pub mod ext_hooks;
pub mod ext_lifecycle;
pub mod ext_rate;
pub mod ext_secure;
pub mod file_ops;
pub mod hooks_bridge;
pub mod mcp;
pub mod op_receipts;
pub mod ops_plan;
pub mod ops_backup;
pub mod output_store;
pub mod permission;
pub mod plugin_manifest;
pub mod registry;
pub mod rel_notes;
pub mod request_perms;
pub mod schema;
pub mod search;
pub mod shell_tool;
pub mod skill_commands;
pub mod skill_gate;
