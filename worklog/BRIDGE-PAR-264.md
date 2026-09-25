# BRIDGE-PAR-264 scratchpad

claim: BRIDGE-PAR-264 via ses_par264, ledger in-progress.
source: crates/opentui-bridge/src/slot_registry.rs:32 SlotRegistry, :16 SlotKind, :44 mount(kind,owner)->bool, :65 unmount(kind,owner)->bool.
target: ONE new file crates/opentui-bridge/src/slot_registry_full.rs. No lib.rs/Cargo.toml/slot_registry.rs edits.
design: SlotFlow { reg: SlotRegistry, ops: u64 }. mount(kind,owner) delegates to reg.mount, ops+=1 on true. unmount(kind) drops first owner of kind via owners_of+reg.unmount, ops+=1 on true. ops() getter.
tests: new-ok, mount-bump, mount-fail-no-bump, unmount-first, unmount-missing, ops-count.
