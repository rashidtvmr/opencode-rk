# NATIVE-MATRIX-RED scratchpad

Claim: NATIVE-MATRIX-RED, session ses_f3c4de578ffelQv59xDXmOs03B.

Base: `7262682e31c1f473912c9483804c9430eb4ee4c5` on
`red/PHASE1-NATIVE-ARTIFACT-MATRIX`.

Source evidence:

- Synthesis stage `NATIVE-MATRIX-RED` assigns
  `tests/bootstrap/test_native_artifact_matrix.py`; later implementation owns
  `scripts/build-native-artifacts.sh`.
- The assigned baseline has no `crates/opentui-bridge/native` directory, no
  `artifacts.json`, no native builder, and no `scripts/*native*` entrypoint.
- Existing later matrix evidence at `phase1-integration` records seven artifacts
  in `crates/opentui-bridge/native/artifacts.json:27-142`: macOS arm64/x64,
  Linux arm64/x64, and a Windows-GNU DLL/import-library pair. It explicitly
  excludes MSVC at lines 171-175.
- Existing later worklog `worklog/TUI-011.md:124-137` says the matrix producer
  is `crates/opentui-bridge/native/build_opentui.sh` pinned to builder commit
  `312d315...`; it also records immutable frozen-test defects. The synthesis
  instead proposes a new top-level script without defining its arguments,
  output directory, offline/source inputs, receipt format, or failure codes.
- External gates `G-LINUX-RUNNER` and `G-WINDOWS-RUNNER` remain blocked; source
  integrity is not native execution proof.

Status: BLOCKED before test authoring. A Python test that merely asserts the
proposed script exists would fail, but would not define observable producer
behavior. Inventing `--print-matrix`, output paths, network/toolchain behavior,
or receipt JSON would create a test-owned API not approved by the packaging
contract. Copying later `artifacts.json` values into a baseline test would test
a manifest owned by `I3-ARTIFACT-MANIFEST`, not the proposed implementation
file, and would misrepresent source hashes as runner proof.

Required authority action: select the canonical producer entrypoint (existing
native builder or approved wrapper) and freeze an implementation-independent
interface covering exact inputs, five GNU/platform outputs, disposable output
root, offline/pinned source behavior, byte/hash receipts, bounded failures, and
explicit MSVC rejection. Rebase onto the approved native-builder/artifact
prewire or add it as a dependency. Then author a static/dry-run RED plus real
native-runner receipts separately.

No test hash was frozen. No product, test, script, manifest, controller, or
verifier file was changed. This blocker keeps `SBOM-RED` and `V1-FREEZE-RED`
open.
