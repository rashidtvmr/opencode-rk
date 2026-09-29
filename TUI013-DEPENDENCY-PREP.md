# TUI-013 dependency preparation candidate

## Candidate identity

- Base candidate: `ffa52ddf4664833bca1fa30fb93ab520fb1d1747`
- Candidate tree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/TUI013-dependency-prep`
- Scope: serialized dependency preparation only; not implementation, acceptance, or controller authorization.
- Owned product file: `crates/opentui-bridge/Cargo.toml`.
- Required integration exception: `Cargo.lock` has the single package dependency edge only.

## Exact proposed diff

```diff
diff --git a/crates/opentui-bridge/Cargo.toml b/crates/opentui-bridge/Cargo.toml
@@
 [features]
 default = []
 native = []
+
+[dependencies]
+rustix = { workspace = true, features = ["termios"] }
```

```diff
diff --git a/Cargo.lock b/Cargo.lock
@@
 [[package]]
 name = "opencode-rk-opentui-bridge"
 version = "0.1.0-alpha.1"
+dependencies = [
+ "rustix",
+]
```

No version, source, checksum, or package addition is proposed. Existing lock entry:
`rustix 1.1.4`, checksum `b6fe4565b9518b83ef4f91bb47ce29620ca828bd32cb7e408f0062e9930ba190`.

## Resolution evidence

`cargo generate-lockfile --offline` was intentionally not retained because Cargo
attempted to resolve cached packages to newer compatible versions (94 lockfile
lines changed), which would violate the no-upgrade constraint. The lockfile was
restored and the exact package edge was added manually for this candidate.

`cargo metadata --offline --locked --format-version 1` was attempted from the
candidate and blocked by the local cache: it tried to download
`android_system_properties v0.1.6`. No network or automatic upgrade was used.

Static lock inspection confirms the pre-existing rustix package remains version
`1.1.4` with the checksum above and the candidate adds only the bridge-to-rustix
edge. `git diff --check` is clean.

## Approval route and unresolved gates

This is a candidate dependency amendment, not self-approval. A serialized
dependency owner must independently review the exact graph/lock delta and run
offline resolution with a complete pinned cache. The normal protected-path route
requires a pull request, `planning`, code-owner review, and one independent
approval; protected-path hosting review must not be bypassed. A separate
independent reviewer must approve this exact graph before any native product lane
implements termios behavior.

No product Rust, frozen tests, claims, policy, registry, or acceptance files were
changed. Native raw-fd/TTY handling remains pending in the product implementation
lane.
