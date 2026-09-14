//! Tool abstractions and built-in tool implementations.
#![forbid(unsafe_code)]

pub mod registry;
pub mod schema;
pub mod executor;
pub mod permission;
pub mod builder;
pub mod skill_gate;
pub mod search;
pub mod request_perms;
pub mod output_store;
pub mod mcp;
pub mod elicitation;
pub mod file_ops;
pub mod shell_tool;
pub mod diff_tool;
