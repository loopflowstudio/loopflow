# Open questions — what goes in the system prompt

## Prerequisites and proposed defaults

The design's **Evidence** section owns the five prerequisite findings, source
links and their limits; its channel table and fill rule own the defaults Jack
Heart authorized for implementation and demonstration. These are not accepted UX.

## Remaining choices and proof gaps

- Jack Heart resolved clipboard placement on October 10: skill, message, then a
  tagged clipboard block, counted toward the first-turn cap. No clipboard reference file.
- Jack Heart's “normal snippet + block thing” is interpreted by the operator as
  marked start excerpts with complete source path/size and a read-the-rest instruction.
  This interpretation needs confirmation at review; shrinking Wave memory remains undecided.
- Small whole files precede excerpts; remaining bytes go to excerpts in the existing
  priority order. Files without room for excerpt metadata remain in the complete listing.
- The callback now uses complete manifest/saved-skill pointers when the reserved
  listing and skill cannot fit together, including combined overflow. Proposed
  default, not accepted UX: demonstrate reading the omitted text through providers.
- Proposed interpretation: “no repository text in system” means no **Loopflow-added** repository
  text: OpenCode's native guide loader itself contributes system content. Suppressing
  that loader would contradict the requirement to preserve harness-owned guides.
- Hook limits cannot count Unicode scalars alone. Both providers preserve 10,000
  ASCII characters but spill/truncate a 10,000-scalar emoji block. Codex keeps
  the markers around missing text; whole-string comparison is required. The
  builder now counts the complete rendered UTF-8 bytes against the shared 10,000-byte
  ceiling. Provider integration must preserve that string without extra unbudgeted text.
- Main #1519/#1520 separate AgentProcess from its attached LfProcess. Context/trust
  must cover supported Codex takeover and native-history replacement without replay;
  existing probes do not cover those paths. LOO-447 owns other-provider takeover.
- Before-compaction scratch writes and Session search remain out of scope.

## Implementation boundary (2026-10-09)

Jack's option A selects argv only and the explicit first-turn cap. Fixed additions,
private source references and native delivery are now connected; they are one PR,
not separately shippable alternatives. The design owns remaining acceptance.

OpenCode's proposed implementation uses native conversation/system transforms,
scoped to the first owning Session; its native startup/compaction matrix is still
unproved. Codex trust hashes are scoped to generated session-flags declarations,
not global approval. Native `hooks/list` confirmed the generated hashes. The
earlier network-isolated startup failed with `Operation not permitted`; Jack Heart
identified the sandbox in October 10 comment `96ee2917` and reported successful
Codex headless and Claude headless/interactive branch launches outside it. Interactive
Codex, compaction, resume and OpenCode remain explicitly nonblocking, unverified coverage.

Jack Heart selected checkout Wave files on 2026-10-09: launch and callback context
read the saved Wave path and its ancestors, listing real paths. Main #1521 removed database document
storage and moved configuration reads to checkout files; its reader is retained.
