# FIX-12 scratchpad

Claim: THEME_NAMES 33 picker names + helpers, names-only.
Source: theme_assets.rs:6-40 ASSET_NAMES (33 sorted basenames); theme.rs:33-111 Theme struct (approximations, not copies); theme_registry.rs:26-105 resolve fns.
Target: crates/opentui-bridge/src/theme_catalog_full.rs only. lib.rs untouched.
Tests: count_is_33, fail_closed_first, has_known_unknown.
Decisions: reuse ASSET_NAMES order verbatim; theme_name OOB => THEME_NAMES[0]; std-only, forbid unsafe.
Unknowns: none. JSON vending out of scope.
