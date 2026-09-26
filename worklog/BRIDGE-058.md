# BRIDGE-058 message_chrome

Claim: crates/opentui-bridge/src/message_chrome.rs. Source: index.tsx:1350-1698 (a0d9b6c).
Reuses crate::message_render::MessagePart, no redefine. No cargo run per scope.
Needs lib.rs `pub mod message_chrome;` + `pub mod message_render;` by integrator (both files exist, neither wired).
