# V2-PROD-NATIVE-ESCAPE-CONTRACT

## Package and status

- Package/gate: production native input salvage / G5 Native TUI.
- Base SHA: `96edb1b18438e1d7311b280374e43ffa8b6e22d9`.
- Candidate status: **CANDIDATE**; runtime execution is intentionally untested
  by this test owner.  This is a frozen source-only contract, not RED,
  PREVERIFIED, or ACCEPTED evidence.
- Allowed writes: `tests/e2e/native_escape_input.py` and this worklog only.

## Frozen observable contract

The installed `oc2` native PTY must preserve a draft containing `native escape
seed`, followed by fragmented CSI `RIGHT` (`ESC [ C`), fragmented SS3 `RIGHT`
(`ESC O D`), a harmless cursor-position report query (`ESC [ 6 n`), and Kitty
Unicode-key `CSI 99;1u` (Unicode code point `c`, press flag 1).  These protocol
bytes must not become literal draft text or submit the draft.  UTF-8 `café😀`
is sent one byte at a time; one scalar backspace must remove only the emoji.
The provider must receive exactly `native escape seed c café` as the first user
prompt, with no assistant history.

The test then opens `/models`, types a filter, sends standalone Escape, and
sends a second ordinary prompt.  The filter must not submit and the provider
must receive exactly `native escape after dialog` as the second user prompt,
with the first user prompt and first assistant response as canonical history.
The test does **not** assert that Escape clears a normal draft, bracketed paste,
or an invented maximum sequence length.

Each control sequence is fragmented to one-byte writes with a 12 ms inter-byte
gap.  This is below the test's documented carry interval and is intended to
prove that a partial sequence is carried rather than treated as standalone
Escape.  The test deliberately does not assert a timing number as product
semantics; the interval is a deterministic harness scheduling value.

## Evidence inspected

- Current pinned product tree, `crates/cli/src/tui_entry.rs`, `native_loop`
  (current checkout lines 560-899): one-byte `read_input`, Kitty keyboard
  enable flag 1, UTF-8 incremental decoder, and current byte-27 behavior that
  clears draft/dialog.  The test is therefore expected to be RED against the
  current loop until a decoder is granted; it does not alter that product file.
- `sources/upstream.lock.json`: OpenCode pin
  `95daf90670b7c039c436c85537da5fbfe2205b41`; repository completeness remains
  explicitly uncertified, so no stronger upstream claim is made here.
- Immutable helper evidence hashes:
  - `tests/e2e/native_utf8_input.py`:
    `f5a4058cc51d6d84e02523633a2601e1bfa741fe8834848353606939a7f80619`
  - `tests/e2e/native_provider_setup.py`:
    `32edfa5c0ac5eacef9cfa949e02fce0d3440fe1cd8f75580efb584bc0a7780dc`
  - `tests/e2e/native_stream_tool.py`:
    `c6d8d561a96136061c4d6e87e183bad144e762df17847fcfca4a347d0d30633e`
  - `tests/e2e/native_interactive_pty.py`:
    `51c37cfe066987ab94b3bba8e8496c7632a37902702e4caf571f8b1dc4ebc9b5`
- Attested release fixture: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-native-workspace-next-96edb1b-ngf18lv1`.
  Its build manifest records the same source SHA, binary SHA
  `18628d30b5ef5968e32172b46819ca92af671de3d34ab2572400d1e4a56d0cbb`, and
  native library SHA `798f30dd7f4be36d52c8834652ed7bcd7f20dfd2a1203d09cc24880eeb13a91`.
  (The manifest is supplied to the runtime command; this source-only handoff
  does not claim those artifacts were executed.)
- `Cargo.lock` SHA-256 before authoring:
  `63ef5299dd93286950af00388796375b06aefc5a4a3eedfa38361954fefb6f03`.

## Harness safety and verification scope

The script requires explicit absolute `--binary`, `--native-library`,
`--build-json`, and `--artifact-dir`; `checked_artifact` validates hashes and
source attestation.  It uses fresh HOME/XDG/data/project directories, a local
loopback Responses fixture with a fixed bearer token, bounded 128 KiB requests,
256 KiB responses, and a 256 KiB PTY capture.  It records semantic errors,
terminal restoration, child reaping, validated daemon disappearance, and
provider-thread join before raising a failure.  Cleanup is attempted on every
failure path and no ambient credentials or user database are used.

Source-only commands run by the author:

```text
python3 -m py_compile -q tests/e2e/native_escape_input.py
python3 - <<'PY'
import ast, pathlib
ast.parse(pathlib.Path('tests/e2e/native_escape_input.py').read_text())
print('AST OK')
PY
git diff --check
```

No runtime RED/GREEN command was run.  The independent verifier must first
review/freeze this source, then run the actual command with the attested
release artifacts.  A future product grant is limited to `crates/cli/src/tui_entry.rs`
plus a new private native input decoder module, serialized through integration.
