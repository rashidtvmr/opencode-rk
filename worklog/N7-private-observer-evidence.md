# N7 private observer evidence

- Package: R1.4 native observer mechanical PTY repair; test owner only.
- Base: `be659402cbc99a4ffdedbfdc461ea754036a9f46`.
- Changed paths: `tests/e2e/native_bracketed_paste.py`; this evidence file.
- Candidate status: source-ready candidate, not native PREVERIFIED or ACCEPTED.
- New test-file SHA-256: `56dd8353f53f2f77c7fd6c8362b7dedfa787809487909821aa7db5ea8024bfe6`.
- Provider helper SHA-256 (unchanged): `32edfa5c0ac5eacef9cfa949e02fce0d3440fe1cd8f75580efb584bc0a7780dc`.
- Historical frozen paste source hash: `6b3c91db2240103c2aa3c13cd2b12831b6a3235891a553f5cd1d0f7d5a134526` (the full original remains in Git).

## Repair

The normal-exit observer now services the PTY master with bounded `select`/`read`
while the child is still running, after Ctrl-C and before the normal
`child.wait`. It uses the existing `MAX_PTY` capture budget and then retains the
existing post-reap EOF drain. EINTR and transient nonblocking reads retry; EIO
after child exit is terminal EOF. On capture overflow or an unexpected read/EOF,
the existing finally path kills and reaps the owned child. No slave ownership,
termios, semantic assertions, protocol bytes, provider helper, or wire
timeouts changed.

## Checks

Passed:

```text
python3 -m py_compile tests/e2e/native_bracketed_paste.py
git diff --check
```

The installed native gate was intentionally not run in this lane. Existing
source-attested gate artifacts must be checked for binding to source SHA and
frozen fixture hash; a new binary must be built by the serialized integrator,
not fabricated from an old `build.json`. The exact old/new native comparison,
manifest, and runtime result remain pending the S1 heavy-build slot.
