# N7 private observer evidence

- Package: R1.4 native observer mechanical PTY repair; test owner only.
- Base: `f6a73d012c3b1bf24e5239eb21cc3096c5c2bb29`.
- Changed paths: `tests/e2e/native_bracketed_paste.py`; this evidence file.
- Candidate status: source-ready candidate, not native PREVERIFIED or ACCEPTED.
- New test-file SHA-256: `144c7d17d771ed3ec6368a4ea8fd2892f02cd3a6a2b35f34690affb869fdb022`.
- Provider helper SHA-256 (unchanged): `32edfa5c0ac5eacef9cfa949e02fce0d3440fe1cd8f75580efb584bc0a7780dc`.
- Historical frozen paste source hash: `6b3c91db2240103c2aa3c13cd2b12831b6a3235891a553f5cd1d0f7d5a134526` (the full original remains in Git).

## Repair

The normal-exit observer now services the PTY master with bounded `select`/`read`
while the child is still running, after Ctrl-C and before the normal
`child.wait`. It uses the existing `MAX_PTY` capture budget and then retains the
existing post-reap EOF drain. EINTR and transient nonblocking reads retry; the
portable `errno.EIO` and zero-byte PTY read are terminal EOF independent of
`Popen.poll()`. Other I/O errors and capture overflow remain failures. No slave
ownership, termios, semantic assertions, protocol bytes, provider helper, or
wire timeouts changed.

## Checks

Passed:

```text
python3 -m py_compile tests/e2e/native_bracketed_paste.py
python3 tests/e2e/native_bracketed_paste.py --self-check
git diff --check
```

Passed an independent private-copy control replay (six finite children, 64 KiB
capture cap, 10-second deadline): old `payload_seen` 0/3; new `payload_seen`
3/3; new helper errors 0/3; new exits `[0, 0, 0]`. The control intentionally
captures only the fixture write, not a claimed write count; its child reports
`WRITECOUNT=1` for each run. Original control: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/n7_control.py`
SHA-256 `b0992198f8507ae43ffb73360a605c537ca58b1e5872d8a8b158bef99d717270`.
Private-copy audit: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/n7-control-eof-fix-audit.json`
SHA-256 `e46fb0dee905a6dcf741aa6b941e2be11749341bb8c3f5025085abbf716aa257`.

This is source-ready/test-only; native runtime remains pending. Exact pending
command: `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/n7-target cargo test -p opencode-server --test native_bracketed_paste -- --nocapture`.
