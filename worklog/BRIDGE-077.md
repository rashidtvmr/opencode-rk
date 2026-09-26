# BRIDGE-077 home_plugin

Claim: passthrough slot registration for home footer + tips.
Evidence:
- footer.tsx:88 `home_footer()` order 100, id `internal:home-footer` (:8)
- tips.tsx:39 `home_bottom()` order 100, id `internal:home-tips` (:7)
- system_plugins.rs:18,299 `ID_HOME_FOOTER`, `HomeFooter`, `register_home_footer` reused
- plugin_slots.rs:61-62 `home_bottom`/`home_footer` `SlotName`, `SlotRegistry` reused
Target: crates/opentui-bridge/src/home_plugin.rs only; lib.rs wiring left to integrator.
Tests: 4 (consts match, footer owner, tips owner, status ready). RED: file absent. Impl: passthroughs + HomePlugin status.
Unknowns: none.
