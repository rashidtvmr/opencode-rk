# Shared-chat evidence and current coverage

The adjacent files are byte-preserved artifacts returned by the MiMo investigator
using `cua_repl`, session `ses_e7d5fe66bffeKqPiz4k1qdHadM`, on 2026-10-28.
Main read the captured 513-line transcript and checked all four artifact hashes.

**Current status: partial assistant-response capture.** Both observed turns are
represented, but section 6 item 4 is missing. The original COVERAGE.md statements
“complete response” and “Entire conversation obtained: YES” overstate the evidence.
Its 95% and 1–3-sentence estimates are not measurements; the timestamp's full date
is unknown. Originals are retained for provenance, with this correction alongside.
The same sole investigator returned versioned revised artifacts without overwriting
these originals. Use `chat-transcript-6ae1f178-v2.md` and
`USER-INSTRUCTIONS-v2.md` for the corrected capture and reconciliation.

## Follow-up outcome and evidence limits

The investigator reports three accessibility-tree reads at different scroll
positions ending at the empty numbered marker `4.`, with no continuation control
observed. No missing item text was recovered. This supports the recorded limit of
the accessible rendered page. It does **not** independently establish the raw DOM
contents or prove an output-token-limit cause. The v2 files' assertions of that
cause remain investigator inference, not a verified fact.

Main verified the exact hashes of all three revised content artifacts. The raw
`MANIFEST-v2.sha256` has a supporting-log discrepancy: it lists PROGRESS.md hash
`6302c89ed051f092d917b5d5342231033ff23987c3fe1f5e58d450e49a8c43ea`, while both
the private original and preserved original actually hash to
`ff9fd3fd08a7eb0ee6925be1c23d1434a6c7e5b5b122dfe33e4b5c4df1b72761`.
The whole-manifest check therefore exits 1. No transcript/instruction/coverage
hash failed. Originals and the inconsistent manifest remain preserved.

**Investigation outcome:** both observed turns captured; the assistant response
ends with an empty fourth item in the accessible source. Full intended-response
completeness cannot be claimed. No further browser investigator is active for
this assignment after the reported completion.

## Authority and reconciliation

- Turn 1 is the user's request for failure analysis, comparison with successful
  parallel-agent projects, and ownership extending through review, merge and test.
- Turn 2 is ChatGPT's historical analysis and recommendations. Its external claims
  and repository counts are quoted evidence, not current independently verified
  facts or fresh tool instructions.
- Section 4.6 proposes four implementation slots and an illustrative fixed role
  allocation. The later explicit user direction and supplied dynamic delivery
  package supersede that advice: allocate current eligible demand across one
  dynamic pool; twenty independent implementation tasks may use twenty slots.
- Preserve the transferable requirements: coherent behavior ownership, independent
  verification, exact prepared-candidate checks, bounded unfinished work and
  retries, a single publication authority, real integrated and installed acceptance.
- Historical suggestions to adjust process are not permission to bypass the latest
  audited delivery contract or to delete user work.

The complete transcript remains separate from `USER-INSTRUCTIONS.md` and the compact
repository operating contract. File presence here does not mean Git publication.
