# Open questions — what goes in the system prompt

## Prerequisites and proposed defaults

The design's **Evidence** section owns the five prerequisite findings, source
links and their limits; its channel table and fill rule own the defaults Jack
Heart authorized for implementation and demonstration. These are not accepted UX.

## Remaining choices and proof gaps

- Wave/ancestor context comes from SQLite (`gather_wave_docs`); the plan now
  preserves that owner and requires readable complete snapshots. Local scratch
  remains live; logical Wave paths must not point at stale checkout copies.
- A listing or active skill can itself exceed the hook cap. Proposed: a complete
  manifest/saved-skill file with a pointer in the block, never truncation. This
  extends the proposal and needs explicit demonstration of reading omitted text.
- Proposed interpretation: “no repository text in system” means no **Loopflow-added** repository
  text: OpenCode's native guide loader itself contributes system content. Suppressing
  that loader would contradict the requirement to preserve harness-owned guides.
- Hook limits cannot count Unicode scalars alone. Both providers preserve 10,000
  ASCII characters but spill/truncate a 10,000-scalar emoji block. Codex keeps
  the markers around missing text; whole-string comparison is required. The
  block builder needs provider-unit accounting before emitting complete files.
- Main #1512 now resumes saved native history on a new engine after driver death.
  Context/trust lifetime must cover that path; restoring the deleted surviving-engine
  recovery is not part of this cut. Existing probes do not cover lf replacement.
- Before-compaction scratch writes and Session search remain out of scope.

## Implementation boundary (2026-10-09)

Jack's option A resolves transport: argv only, with an explicit first-turn cap.
The Rust first-turn/measurement slice is not channel-contract completion. Remaining
fixed-slot, whole-file block, native hook/trust and terminal additive-instruction
work is listed in the design; old system-file context cannot count as acceptance.

Compression interpretation (2026-10-09): size-target inspection no longer previews
skills or Task messages, so remove `lf context --skill`, goal-status and duplicate
original/submitted counters. Preserve measured context usage and DTO unknowns;
remove only the retired assembled-budget fields. The fixed-slot/conversation-block
cut remains incomplete and cannot ship on these reductions alone.
