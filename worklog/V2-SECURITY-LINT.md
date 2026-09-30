# Security library Clippy maintenance

Base: `4a3e98e0cc9bb22b8b142d28558daceea19c81a2`.

Candidate: `482e5a13f5a9ff579ab79e2ef82bf3caec0e88d0`.

This candidate makes only diagnostic-preserving Clippy repairs in the nine
owned security source files. It changes no semantic tests, manifests,
configuration, authority files, or product behavior. Repairs are limited to
redundant field syntax, unused imports, a const assertion, equivalent boolean
branching, `rfind`, derived `Default` with `InheritAll` explicitly marked as the
existing default, `contains`, an identity cast, and `&Path` helper parameters
with `to_path_buf` at the owned-value boundary.

The initial formatting command accidentally touched unowned workspace files;
those paths were immediately restored before handoff and are not part of the
candidate commit. An initial bounded verification attempt exposed the
mechanical `&Path` conversion omissions in the three helper bodies; those are
corrected in the follow-up candidate. Parent must independently rerun the
bounded Clippy and security-library test commands.
