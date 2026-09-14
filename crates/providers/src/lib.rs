//! Provider abstractions for LLM and tool backends.
#![forbid(unsafe_code)]

pub mod auth;
pub mod budget;
pub mod config;
pub mod cost;
pub mod debug_export;
pub mod fallback;
pub mod health;
pub mod integration;
pub mod metrics;
pub mod model_route;
pub mod rate_limit;
pub mod registry;
pub mod retry;
pub mod router;
pub mod streaming;
pub mod tap;
