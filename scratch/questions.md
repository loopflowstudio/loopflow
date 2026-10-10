# Open questions — what goes in the system prompt

## Prerequisites and proposed defaults

The design's **Evidence** section owns the five prerequisite findings, source
links and their limits; its channel table and fill rule own the defaults Jack
Heart authorized for implementation and demonstration. These are not accepted UX.

## Remaining choices and proof gaps

- Wave/ancestor context comes from SQLite (`gather_wave_docs`); the plan now
  preserves that owner and requires readable complete snapshots. Local scratch
  remains live; logical Wave paths must not point at stale checkout copies.
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
- Main #1512 now resumes saved native history on a new engine after driver death.
  Context/trust lifetime must cover that path; restoring the deleted surviving-engine
  recovery is not part of this cut. Existing probes do not cover lf replacement.
- Before-compaction scratch writes and Session search remain out of scope.

## Implementation boundary (2026-10-09)

Jack's option A selects argv only and the explicit first-turn cap. Fixed additions,
private source references and native delivery are now connected; they are one PR,
not separately shippable alternatives. The design owns remaining acceptance.

OpenCode's proposed implementation uses native conversation/system transforms,
scoped to the first owning Session; its native startup/compaction matrix is still
unproved. Codex trust hashes are scoped to generated session-flags declarations,
not global approval. Native `hooks/list` confirmed the generated hashes. The
network-isolated actual lf/native startup attempt failed with `Operation not
permitted` before any model request; cause remains unresolved and gate owns it.

Saved delivery follows the Wave ID across renames while scratch stays in its
captured checkout. The rename/name-reuse fixture passes; native continuation is
still with gate. No filesystem Wave fallback is selected.
