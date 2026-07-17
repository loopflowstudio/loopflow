# Open implementation notes

- This pass takes the first executable cutover slice: dynamic same-Turn Send
  outcomes, portable Steer fallback, and trace vocabulary. The Work/Epoch/Run
  persistence migration remains a later checkpoint; adding a second dormant
  runtime beside Sessions would violate the no-dual-architecture rule.
- Project and Task still persist live delivery through `ChildCommand` until the
  Steer/Send migration. The controller now keeps every live, rejected, failed,
  or unknown Steer in the next-boundary seed, but crash-proof incorporation
  still depends on replacing that ledger with immutable Send plus Basis.

## One spend grain (W2-280): what the dogfood ledger proved

Measured on a copy of `~/.lf/loopflow.db` before cutting `run_events` spend:

| | `run_events` | `agent_turns` |
| --- | ---: | ---: |
| usage-bearing rows | 103 / 181,806 | 779 / 1,228 |
| output tokens | 1,428,413 | 3,599,965 |

The two ledgers were never complementary. Every usage-bearing process in
`run_events` (75/75) also had a captured turn, over the identical date span, so
the exec ledger was a strict subset that saw ~40% of the spend. `lf usage` and
`lf top` both read that subset.

The exec ledger also mis-attributed. `record_agent` stamped a thread-local with
whichever agent launched *last* in the process, while `record_usage` accumulated
tokens from every launch and drained them at the terminal boundary. One process
that ran claude/opus (skill `rebase`) and then opencode/glm-5.2 therefore
reported claude's 40/5,197 tokens under `provider = opencode`. Per-launch
attribution is only correct at the grain the launch owns — the Turn. Cutting to
the turn join moves exactly those 5,197 tokens back onto claude.

Nothing was lost: the other seven opencode launches carry no usage in *either*
ledger.

## Open: OpenCode turns report no usage (W2-289)

OpenCode's genuinely-unreported usage is now visibly absent instead of being
mis-attributed to another provider's row. Eight opencode launches from
2026-07-16 are `capture_status = complete` with zero turns carrying usage.

The cause is upstream of this slice and untouched by it: `TraceCapture` gets
usage from two parsers, and neither reaches a headless opencode launch.

- `StreamEvent::Usage` (`engine/stream.rs`, accumulates with `+=`) reaches a
  capture only through `engine/agent.rs`'s stream/batch launch path.
- `ConversationEvent::TurnUsage` (`harness/opencode_mapping.rs`, replaces with
  `=`) reaches a capture only through `flowloop/wave.rs:835` — the Wave
  resident. It is the *only* caller of `capture.record_conversation`.

So a Task/Project runner launching opencode has no path that sets
`usage_observed`, and `apply_usage_to_turn` writes nothing. Deciding this needs
its own evidence per harness and launch surface: which of the two parsers should
own usage end to end, and how the harness event stream reaches a capture outside
the Wave resident. Do not collapse the two parsers by deleting one arm without
that evidence — they serve different launch surfaces, not one duplicated path.

## Stale after this slice, not editable here

`wave/intelligence/MEMORY.md:184` records "**`run_events` is the one home for
token and cost evidence**". That decision is superseded — the one home is
`agent_turns`, joined through `agent_launches`. Wave memory is server-owned, so
it is flagged rather than edited.
