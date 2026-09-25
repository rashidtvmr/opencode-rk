# SETUP-RED scratchpad

Claim: SETUP-RED, session ses_f3c4de578ffelQv59xDXmOs03B.

Base: `7262682e31c1f473912c9483804c9430eb4ee4c5` on
`red/PHASE1-ACCOUNT-SETUP`.

Source evidence:

- Current pure setup model, `crates/providers/src/account_setup.rs:1-8`, states
  that it performs no I/O and that the caller owns persistence and transport.
- `PendingSetup::complete` at `:160-182` accepts caller-supplied booleans for
  credential validity, endpoint reachability, and model availability; no
  credential entry, broker authorization, validation transport, or secure-save
  caller crosses this boundary.
- `AuthorizedAccount` at `:38-77` always reports secure storage because it holds
  a zero-sized `SecureStoreMarker`; callers do not provide evidence of a
  successful secret write.
- Existing unit tests at `:475-610` already cover cancellation, consent,
  validation booleans, bounded accounts, removal receipts, and offline markers.
- Synthesis says `SETUP-IMPL` must provide brokered credential input while
  durable OS-secret persistence remains blocked by `G-KEYRING-SEAM`, but gives
  no input broker trait, secret lifetime, caller, or save/load contract.
- The keyring seam audit at `worklog/PHASE1-KEYRING-SEAM.md` requires a real
  production save/load caller before adding keyring and preserves the exact
  future pin `keyring = "=3.6.3"`.

Status: BLOCKED before test authoring. Public setup APIs can only duplicate
already-GREEN pure-state tests or trust fabricated booleans/markers. There is no
implementation-independent compiling seam through which a test can provide a
bounded secret, observe broker authorization, prove zeroized lifetime, or verify
that success follows a real secure-store write. Inventing a trait or callback in
the test would define the implementation API; reading environment/files would
violate the security contract.

Required authority action: prewire the production setup caller and a secret-safe
interface defining provider/account identity, broker operation, maximum secret
bytes, cancellation/zeroization lifetime, validation transport, and the point at
which an account becomes authorized. Durable save/load remains a separate RED
behind `G-KEYRING-SEAM` and must pin keyring exactly if approved. The setup RED
can then use a fake bounded issuer/store in disposable memory without real
credentials and assert deny/cancel leave no account or retained secret.

No test hash was frozen. No product, test, Cargo, manifest, controller, or
verifier file was changed. This blocker also prevents selecting
`MULTICLIENT-RED`, whose synthesis dependency is `SETUP-RED`.
