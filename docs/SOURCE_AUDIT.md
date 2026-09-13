# Source evidence and completeness audit

DISC-001 freezes immutable upstream references before any feature-parity claim. `tools/freeze_sources.py --fetch` accepts only credential-free public HTTPS GitHub URLs and exact locked commit, tree, and license blob identities. It never falls back to a branch, recursively initializes submodules, installs packages, or runs upstream lifecycle scripts.

Git fetches run with prompts disabled, global/system Git config disabled, an empty temporary HOME, hooks redirected, bounded retries, bounded subprocess time, and no ambient credentials forwarded. Existing or newly fetched checkouts must verify exact `HEAD`, exact `HEAD^{tree}`, exact `origin`, clean worktree including untracked files, detached HEAD, Git object connectivity, and exact license blob identity.

After every locked repository verifies, the freezer atomically writes `.upstream/frozen-manifest.json`. The manifest contains only public source identity and verification properties; local paths, HOME, tokens, helpers, and other ambient secrets are excluded.

`tools/inventory.py` re-verifies each checkout rather than trusting prior freeze output. It emits JSONL tracked-object inventories and a schema-v2 manifest containing commit, tree, origin, license identity, connectivity, detached/clean state, counts, and hashes. `tools/coverage_gate.py` compares that inventory identity back to `sources/upstream.lock.json` and fails closed on stale, partial, attached, dirty, or unreviewed inputs.

Candidate ownership globs are discovery hints only. Every tracked entry still needs an explicit reviewed disposition and product behavior mapping or a justified non-product rationale. Behavior-surface coverage remains a separate ledger and independent verification obligation.

DISC-001 is not accepted yet: this execution shell cannot fetch the real pinned repositories. Local tests verify fail-closed identity, retry, credential, and manifest behavior, but a trusted network-enabled run must still produce and authenticate the real frozen and inventory manifests.
