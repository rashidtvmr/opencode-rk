# V2 provider auth source contract

Package: independent G2 auth-source contract/test owner (PROV-030). Base: `539cc61f2562b456e6732883da6d9dbbc40d8660`.

This package adds only `crates/server/tests/prov_030_auth_sources.rs`; it does not modify production code, frozen tests, or lockfiles. The installed-daemon tests exercise source selection before outbound provider authorization and assert terminal success plus the actual captured provider `Authorization` value.

## Authority

Pinned upstream `/Users/mymac/Projects/opencode-upstream-reference/packages/opencode/src/auth/index.ts`: `Api` has required string `key` and optional `metadata: Record<string,string>` (lines 23-27); `all` parses non-empty `OPENCODE_AUTH_CONTENT`, falls back to file on JSON parse failure, and otherwise returns the parsed inline object without merging the file (lines 58-67). File records are schema-filtered. Provider auth persistence over ambient environment is evidenced by `provider/provider.ts:1582-1610`.

## Contracts

1. Invalid inline JSON falls through to a valid persisted record.
2. Empty inline content falls through to a valid persisted record.
3. Valid inline API auth overrides persisted and ambient credentials.
4. Valid empty inline object replaces the file; ambient is then used, not the file.
5. A selected file API record with non-string metadata is rejected and ambient is used.
6. A valid selected file API record survives a malformed sibling and wins over ambient.

All fixtures use synthetic secrets, `env_clear`, disposable HOME/XDG/data/project directories, bounded readiness and I/O, a short Unix-socket root, and RAII child/provider cleanup. `OC2_TEST_BINARY` is mandatory.

## Mechanical fixture correction

The provider now restores accepted sockets to blocking mode before reading, parses bounded HTTP headers and `Content-Length` bodies without waiting for EOF, and enforces a one-request provider fixture. Daemon readiness installs an RAII child guard immediately after spawn, polls with a bounded sleep, and uses a fixed minimal executable PATH. The daemon response uses the same bounded framing parser. These are fixture-only corrections; all six semantic status and Authorization assertions remain unchanged.

## Verification status

No Cargo/build/test command was run in this slot by instruction. Parent must compile/run the focused installed-daemon gate with the approved short temporary root and installed candidate binary, using the server crate package (`-p opencode-rk-server --offline --locked`), then freeze the source hash and capture RED/GREEN results.
