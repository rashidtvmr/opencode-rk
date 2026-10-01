# V2 ownership fixture maintenance

**CANDIDATE DONE; integrated gate PENDING**

## Package and revision

- Package: `V2-OWNERSHIP-FIXTURE-MAINTENANCE`.
- Gate: `G4-SESSION-EXECUTION-SERIALIZATION`, the existing five live controls in
  `session_execution_ownership`.
- Role: independent mechanical TEST OWNER, explicitly delegated by the current
  requirement; the parent is the sole integration writer and runtime verifier.
- Base SHA: `1bd286a9a4ac1193185a11d8443f68a8adc50b21`.
- Branch: `v2/owner-fixture-maintenance-y52o0gi3`.
- Candidate SHA: the commit adding this worklog, reported as a full SHA in the
  handoff; resolve with
  `git log -1 --format=%H -- worklog/V2-OWNERSHIP-FIXTURE-MAINTENANCE.md`.
- Exact changed paths:
  - `crates/server/tests/session_execution_ownership.rs`
  - `worklog/V2-OWNERSHIP-FIXTURE-MAINTENANCE.md`
- Lifecycle: `CANDIDATE`; runtime PREVERIFIED and integrated ACCEPTED remain pending.

Read `AGENTS.md`, `PLAN.md`, `docs/CONVERGENCE.md`,
`docs/AGENT_STRATEGY_V2.md`, `docs/TDD.md`, and `docs/SECURITY.md` before editing.
The worktree was clean on the declared base before this repair.

## Failure and authority evidence

The inspected integrated artifact is:

`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-server-quality-integrated-1bd286a-l8jwkqui/agent-loop-and-ownership.log`

Its lines 9-12 show the agent-loop control passing. Lines 16-21 and 25-35 show
four ownership controls passing and
`same_session_nonstream_then_stream_is_serial` failing at the provider fixture's
first request read (base line 253):

```text
provider request: Os { code: 35, kind: WouldBlock, message: "Resource temporarily unavailable" }
provider request worker failed after all workers joined
provider server thread failed
```

The aggregate artifact therefore has five passing controls and one failure.
The current requirement also records a recurrence in the opposite mixed mode
on the earlier `15179...` integrated revision and an earlier candidate with all
five ownership controls passing. These observations motivate deterministic
fixture readiness rather than another chance rerun.

Current explicit authority requires restoring blocking mode on accepted provider
sockets, retrying `WouldBlock`/`TimedOut`/`Interrupted` within the existing absolute
five-second deadline, and preserving all semantic controls byte-for-byte.
`AGENTS.md` test policy and `docs/CONVERGENCE.md` authorize independent mechanical
fixture maintenance without changing asserted behavior. The existing Rust fixture
is the contract evidence for this repair.

Source/socket evidence independently inspected:

- Base fixture `Provider::start` (lines 142-169): the listener is nonblocking;
  accepted sockets are passed directly to concurrent workers, with a two-worker
  cap. `provider_request` (lines 247-253) sets timeouts but does not explicitly
  restore blocking mode. `read_request` (lines 323-331) has an absolute deadline
  but propagates the first `read` error immediately.
- Local macOS SDK manual
  `/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk/usr/share/man/man2/accept.2`,
  lines 59-64: `accept` creates a socket with the listener's properties.
- The same SDK's `read.2`, lines 176-190: nonblocking reads with no ready data
  return `EAGAIN`; a read interrupted before data arrives returns `EINTR`.
- The same SDK's `getsockopt.2`, lines 314-332: `SO_RCVTIMEO` limits input waits
  and can return `EWOULDBLOCK` with no received data; it is an inactivity timer,
  so an independent absolute deadline is necessary.
- Rust 1.98.1 `TcpStream` documentation, read as API evidence:
  <https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.set_nonblocking>
  describes entering/leaving nonblocking mode and transient `WouldBlock`;
  <https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.set_read_timeout>
  documents platform-dependent `WouldBlock` versus `TimedOut` and rejects zero
  timeouts. The installed toolchain manifest also identifies Rust 1.98.1.

## Scoped mechanical repair

The fixture diff has exactly two readiness hunks (25 added lines, two removed):

1. At the start of `provider_request`, call `set_nonblocking(false)` before the
   configured read/write timeouts.
2. In `read_request`, compute the remaining budget from the original, once-created
   five-second deadline. Reject a zero remaining budget using the existing
   deadline error; before each read set the blocking timeout to
   `min(remaining, 500 ms)`. Retry only `WouldBlock`, `TimedOut`, and `Interrupted`,
   sleeping for `min(remaining after the read, 2 ms)` before the next deadline
   check. Other errors retain immediate propagation.

Thus a partial request or transient read does not grant another five seconds;
the last blocking read is configured for the remaining budget rather than a
fresh 500 ms. Transient retries have bounded sleeping instead of busy spinning.
The parser retains its existing 4096-byte buffer, absolute deadline, EOF failure,
content-length validation, and `MAX_WIRE = 128 * 1024` bound.

Source comparison preserves the concurrent provider worker cap, observation
windows, asynchronous first-start/partial readiness, provider release, all
request/response/history expectations, and cleanup. In particular,
`Provider::start` still joins every request worker before its aggregate failure
assertion, `Provider::drop` still releases/stops/joins the provider thread during
unwind, and the existing `ServerTask` cleanup is identical.

## Hashes and unchanged semantic proof

Whole-file SHA-256:

```text
old frozen: 9fdb5a6c01eb7d663421956fda1cc6c5bbf3ffcd43f7873f44bfb8e1f1cbb70c
candidate:  d77d6c538ea0e18b61bc7fd93abd40d0acd4960bb1f6909b948928dc32f76a4b
```

The old frozen bytes remain available at the base commit in Git history. The
new hash records this independent mechanical supersession candidate; the parent
freezes the replacement only after the candidate and exact integrated gates pass.

An executed source-only Python check reversed precisely the blocking-mode
insertion and the deadline/read retry replacements and compared the result to
`git show 1bd286a9a4ac1193185a11d8443f68a8adc50b21:crates/server/tests/session_execution_ownership.rs`.
The reconstructed file equalled every original byte. No assertion occurs in the
changed readiness tokens, proving all existing assertions remain byte-for-byte
identical, including their strings and request/history conditions.

The same check directly compared these protected regions in both revisions:

| Region | Identical bytes | SHA-256 in both revisions |
| --- | ---: | --- |
| File start through `Provider::drop`, before `provider_request` | 8073 | `2c89355f234540d4f770d3ac8084f025d678d286347475eae4b8815657e330ec` |
| Provider request checks/counts/release/response, after `read_request` invocation | 2969 | `ddecea1f05204a0e77372d7bfa78d8aba404d99df8223af2e17c1166b72cdef9` |
| Request EOF/wire/content-length parser, from `if n == 0` | 1375 | `186794f78b84d8636c788b3566dc9efd918685c1de17cc3893264234080e4770` |
| `read_response` through EOF, including clients/history/tasks/harness/semantic helpers/tests | 14406 | `bd13036983dd7f44cfe4e7840555fe2a339a393a075752f6eba1489becdd3388` |

Each test declaration and body, including its attribute and trailing whitespace,
was additionally compared directly and hashed:

| Existing live control | Identical bytes | SHA-256 in both revisions |
| --- | ---: | --- |
| `same_session_two_streaming_turns_are_serial` | 157 | `71bbdd7ce8864d339e8d6826fae9b96e9906bae8614b158fdb7a3fc06463ee53` |
| `same_session_nonstream_then_stream_is_serial` | 159 | `1565bfa592f8b04dfbba36319289090d39cea3e024ffaa9fb12518319d5be1fd` |
| `same_session_stream_then_nonstream_is_serial` | 159 | `aa1a996eaf1a2a0a778e0ce2065da5ca80ab96d8f8295f599bfe5b5a5fb10306` |
| `different_sessions_can_use_two_provider_slots` | 162 | `ddac35e453337925c418d1bb8d6804027f75e7072ace7a0a9623d85f6891fabf` |
| `unauthenticated_second_client_has_no_provider_side_effect` | 2174 | `48c2cc1ae3baeb2a5ff6ef37c2adf15becda859d65d68beffc8a0b2618b48b69` |

## Verification and parent handoff

Executed in `/Users/mymac/Projects/opencode-rk-v2-owner-fixture-y52o0gi3`:

| Exact command/check | Result |
| --- | --- |
| `git rev-parse HEAD` | Declared base SHA above |
| `git status --short --branch` before editing | Clean declared branch |
| Exact source-only Python command reproduced below | Exit 0; old/new hashes verified; four protected regions and all five test declarations/bodies identical |
| Source-only inverse-readiness-patch byte comparison using Python, `git show`, `Path.read_bytes`, and `hashlib.sha256` | Exit 0; removing precisely the two readiness hunks restores every base byte |
| `rustfmt --check --edition 2021 crates/server/tests/session_execution_ownership.rs` | Exit 0 after formatting the new blocking-mode statement |
| `git diff --check` | Exit 0 |
| `git diff -- crates/server/tests/session_execution_ownership.rs` | Exactly the two described readiness hunks |

Reproducible independent region/body proof (source inspection only):

```sh
python3 - <<'PY'
from hashlib import sha256
from pathlib import Path
import re
import subprocess
base = '1bd286a9a4ac1193185a11d8443f68a8adc50b21'
path = 'crates/server/tests/session_execution_ownership.rs'
old = subprocess.check_output(['git', 'show', f'{base}:{path}'])
new = Path(path).read_bytes()
assert sha256(old).hexdigest() == '9fdb5a6c01eb7d663421956fda1cc6c5bbf3ffcd43f7873f44bfb8e1f1cbb70c'
assert sha256(new).hexdigest() == 'd77d6c538ea0e18b61bc7fd93abd40d0acd4960bb1f6909b948928dc32f76a4b'
for start, end in [
    (None, b'fn provider_request('),
    (b'    let request = read_request(&mut stream)', b'fn read_request('),
    (b'        if n == 0 {', b'fn read_response('),
    (b'fn read_response(', None),
]:
    def region(source):
        first = source.index(start) if start else 0
        last = source.index(end, first) if end else len(source)
        return source[first:last]
    assert region(old) == region(new)
pattern = rb'(?m)^#\[tokio::test\([^\n]+\)\]\nasync fn (\w+)\('
def tests(source):
    found = list(re.finditer(pattern, source))
    return {
        match[1].decode(): source[
            match.start():found[i + 1].start() if i + 1 < len(found) else len(source)
        ]
        for i, match in enumerate(found)
    }
assert len(tests(old)) == len(tests(new)) == 5
assert tests(old) == tests(new)
print('PASS: hashes, protected semantics, and all five test bodies unchanged')
PY
```

Parent-required gate, run on the exact candidate SHA and again on the exact
integrated SHA:

```sh
cargo test --offline --locked -p opencode-rk-server --test session_execution_ownership -- --test-threads=1 --nocapture
```

All five existing live controls are required. Runtime validation is reserved for
the parent; this leaf ran source, formatting, and diff checks only. The last
observed runtime result remains the integrated artifact's four ownership passes
and one `WouldBlock` fixture failure. Candidate GREEN, integrated GREEN, and the
replacement mechanical-hash freeze are pending.
