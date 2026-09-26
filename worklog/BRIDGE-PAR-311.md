//! Scratchpad BRIDGE-PAR-311: full ASCII logo banner.
//! Claim: ses_par311. Source evidence: logo.ts:1-11 (logo/go/marks),
//! logo.tsx:1-61 (marks decode + side-by-side). Target: logo_full.rs only.
//! Boundary: std-only, forbid unsafe, <70 lines. Tests: 4 in-file, unrun (no cargo per scope).
//! Decision: plain-ASCII 3-row banner (no backslashes) + text/width helpers; full wordmark stays in logo_art.rs.
