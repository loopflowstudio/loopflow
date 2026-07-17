# Open implementation notes

- Stored Review/Handoff and ChildCommand are gone. The remaining core cut is
  exact Run credentialing, parent attention scheduling, durable root Turn
  output, and deletion of the Project/Task Session-body controller.
- `ambient_run_lease` still derives authority from Session id + generation +
  body token, and a missing legacy bundle can become User. The next pass uses
  one opaque `LF_RUN_LEASE` whose hash locates the exact active Run and fails
  closed.
- Main's account-lease broker confirms the capability semantics: resolve once,
  inherit one fixed opaque grant, prevent nested widening, and fail closed.
  Run lease validation stays local to SQLite; it does not need another SSH
  broker.
- Review attention currently contains no child utterance. Persist optional root
  assistant text on Turn and project it with current child facts into the
  parent control seed. This avoids both a Message aggregate and an unusable
  content-free attention signal.
- Main's draft scripts/docs refer to a Rust `DRAFTS` registry that was not
  landed. The six unpublished architecture migrations must become drafts, but
  fresh test databases still need one coherent way to apply them before the
  release cut. Do not invent a second durable migration ledger.
- The branch has already exceeded the normalized 12,000-line deletion target.
  Restore focused CI/control behavior tests even if the physical count rises.

## Codex steer rejections: what the live app-server proved

Probed against codex-cli 0.144.5 (`codex app-server`, real JSON-RPC, no
`turn/start` needed for the first two):

| Request | Response |
| --- | --- |
| steer an idle thread | `-32600` `no active turn to steer` |
| steer with a stale `expectedTurnId` while a turn is live | ``-32600`` ``expected active turn id `X` but found `Y` `` |
| steer a thread that does not exist | `-32600` `thread not found: <id>` |
| malformed params | `-32600` `Invalid request: invalid type: null, expected a string` |

**One code covers all four.** Classifying by JSON-RPC code is therefore
impossible; only the message separates provider policy from a Loopflow defect.
`send_current` now matches the two policy shapes and defaults everything else to
`Failed`, so an unrecognized error stays loud instead of being absorbed as a
normal seed fallback. This is brittle against vendor prose changes — the
mitigation is the default, not the match: a reworded rejection degrades to a
noisy `Failed` that still seeds correctly, never to a silent wrong answer.

Worth noting from the probe: steering with the *correct* `expectedTurnId` two
seconds after observing it still returned `no active turn to steer` — the turn
had already ended. The Turn-boundary race the architecture predicts is not
theoretical; it is the common case, and it was previously logged as `Failed`.

Not yet observed: a *successful* steer response. The probe could not catch a
live turn fast enough to confirm the `result.turnId` shape that `Sent` depends
on. That shape is still assumed from the app-server README.

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
