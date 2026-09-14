//! Provider abstractions for LLM and tool backends.
#![forbid(unsafe_code)]

pub mod registry;
pub mod config;
pub mod rate_limit;
pub mod retry;
pub mod streaming;
pub mod auth;
pub mod health;
pub mod metrics;
pub mod router;
pub mod fallback;
pub mod budget;
pub mod tap;
pub mod debug_export;
