# Parity harness Cargo target (handoff)

This directory is owned by UI-parity package P01. The harness itself is
Python-only and runs today without a Cargo target:

```sh
python3 tools/ui_parity/selftest.py
```

## Exact Cargo.toml stanza for the integration writer

Do **not** add this in the P01 change (P01 must not edit `Cargo.toml`). When the
integration writer wants the harness in `cargo test`, add this stanza to
`crates/cli/Cargo.toml`:

```toml
[[test]]
name = "ui_parity_harness"
path = "tests/parity/ui_parity_harness.rs"
required-features = ["native"]
```

The test target should be a thin process wrapper, not a second implementation:

```rust
// crates/cli/tests/parity/ui_parity_harness.rs
use std::path::Path;
use std::process::Command;

#[test]
fn ui_parity_comparator_self_test() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest_dir
        .ancestors()
        .nth(2)
        .expect("repository root");
    let status = Command::new("python3")
        .arg("tools/ui_parity/selftest.py")
        .current_dir(repo)
        .status()
        .expect("python3 must be available for the parity harness");
    assert!(status.success(), "ui parity self-test failed");
}
```

That test only proves the comparator and recording pipeline. Fixture goldens are
checked with:

```sh
python3 tools/ui_parity/ui_parity.py --check --filter 'par-static-*'
```

The native PTY capture needs the platform OpenTUI dylib. The adapter sets
`DYLD_LIBRARY_PATH`/`LD_LIBRARY_PATH` from
`crates/opentui-bridge/native/lib/<arch>-<os>/` automatically. If the dylib is
absent, use the `oc2-once` profile for an explicitly unstyled capture; it will
not satisfy styled goldens.
