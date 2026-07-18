# Open implementation notes

## Gate audit 2026-07-17

`uv run python scripts/test.py --all` now passes Python, website, Swift, E2E,
fmt, clippy, Swift boundary checks, and the Mac build-for-testing gate. A
no-fail-fast Rust run is 1,814 passed / 1 failed / 2 skipped. The sole failure is
the controller split already named below:

`task_github_cache_tests::rest_failure_opens_one_durable_circuit_while_local_controls_continue`
constructs a live legacy Task body whose mirrored active Run has no product
Launch, so direct Run interrupt returns `Query returned no rows`. Do not add a
Session interrupt fallback. Every live Task/Project executor must become a Run
Launch in the shared controller cut.

Gate fixes kept on this branch:

- customer docs and builtin skills no longer teach deleted Handoff, Review,
  command-receipt, acknowledgment, or decision surfaces; a builtin test guards
  the retired commands;
- integration and E2E tests clear inherited Run/Session authority and isolate
  their Loopflow homes instead of touching the invoking agent's development DB;
- successor tests now assert stable Work/current Epoch semantics rather than
  separate Session-era control seeds;
- the Wave command matrix supplies the explicit radio channel value.

Current measures: 122,944 Tokei Rust code lines (1,125 over the 121,819 target),
19 production files and 567 references across `ProjectSessionStatus`,
`TaskSessionStatus`, and `ChildWriteLease`. These are the remaining controller,
not cleanup noise.

## Resume point verified 2026-07-17 (HEAD 5119f1791, tree clean)

Green baseline this run: `cargo build -p loopflow`, `cargo fmt --all --check`,
`cargo clippy -p loopflow --all-targets -D warnings`, and 1,555 lib tests all
pass on the committed tree. The "add code in the working tree pending" phrasing
in `architecture.md`/`implementation-plan.md` is now stale: the Review handshake,
parent scheduling, and exact Run lease are **committed**, not uncommitted, and
proven by tests already in the tree —

- handshake: `store/mod.rs` proves parent Steer clears only `attention_at`,
  re-entering the flow does not re-arm, the child's next terminal Turn re-arms
  once and allocates exactly one parent evidence revision, and `close_review`
  clears the route; `only_the_active_parent_run_can_steer_child_work` fences the
  stale/wrong lease.
- scheduling: `flowloop/wave.rs` `seed_only_wave_services_child_once_without_advancing_background`
  and `live_wave_preempts_background_for_child_and_preserves_playhead`;
  `task/runner.rs` routes user vs parent attention.
- exact lease + settlement: `LF_RUN_LEASE` resolves the active Run; CI settlement
  records the first fresh repaired head.

**The one remaining cut is the Session/body controller deletion, and it has no
safe separable sub-slice for a headless pass.** Confirmed this run:

- `advance_run`/`reserve_run`/`stop_run` drive only Wave (`flowloop/wave.rs`) and
  the durable store API. `task/runner.rs` and `project_session/runner.rs` never
  call them — Task/Project execution is still entirely the legacy `ChildWriteLease`
  Session-generation path.
- `ChildWriteLease` and its env readers are load-bearing across every live child
  store op (`store/child_sessions.rs`, `store/sqlite/child_sessions.rs`,
  `task/runner.rs`, `project_session/runner.rs`, `ops/task.rs`). Not vestigial.
- `child_control.rs` (incl. `absorb_run_control`, `send_outstanding_steers`) is
  the live bridge both runners call — not dead, not separable.
- Already at zero, do not recreate: `Sleeping`, `HandleId`, `Actor`, `Ack`,
  `finish(run)`, and all Interaction/Handoff/ChildCommand nouns.

Precise resume plan for the next (supervised, non-headless) pass, in order:
1. Make `task/runner.rs` execute one boundary through `reserve_run`/`advance_run`/
   `stop_run` instead of the Session generation, keeping domain closure/PR/CI/flow
   selection behind a `WorkRef` match. Delete its `ChildWriteLease` threading.
2. Repeat for `project_session/runner.rs` behind the same shared controller.
3. Collapse `ops/task.rs`/`ops/project.rs` Session-shaped control DTOs onto Run
   controls; drop the ambient `*_write_lease_from_env` authority.
4. Delete `child_session.rs` lease machinery, `store/child_sessions.rs`, and
   `store/sqlite/child_sessions.rs` once no live writer needs them; the final
   schema cut copies retained domain facts and drops the Session lifecycle/process
   columns.
5. Restore the CI claim/preemption focused proofs the deleted 2,767-line suite
   covered (exact-Run claim rejects an overlapping repair Run; stale/land-time
   incidents do not preempt; a parked Review is preempted at most once).
   **Partly done headless (2026-07-17):** the runtime wake/preempt *gate* now has
   a focused proof —
   `task::runner::tests::current_ci_incident_warrants_a_wake_only_for_a_fresh_repairable_failure`
   pins `current_ci_incident`, the single mint point both the runner's
   `current_ci_incident_identity` review-preempt check and the idle
   `arm_ci_fix_wake` arm consult: a fresh repairable failure warrants an
   incident; green, a stale reading for a past head, and a land-time-only
   (`scratch-clear`) head warrant nothing; a mixed land-time+real head still
   warrants it. That closes the "(c) does-not-preempt at integration altitude"
   gap without a mock-heavy loop drive. Still open and reserved for the
   supervised cut: the **preempt-*once*** property (`review_preempted` is pure
   `run_task_child` loop state, cleared on `TurnCompleted`, so proving it needs
   the loop harness — do not extract the inline gate solely to test it), and the
   exact-Run claim rejecting an overlapping repair Run at runtime.

This is a live-execution-path replacement of the two most critical runners; a
partial version is exactly the dual-write checkpoint the design forbids and is
unsafe to land unsupervised. Do it as one supervised cut, not a headless slice.

## Migration DRAFTS reality (verified 2026-07-17)

There is no runtime `DRAFTS` registry. `store/migrations.rs` is a hardcoded
`Migration { include_str!(...) }` array applied by canonical ordinal;
`store/migrations/drafts/` holds only a README. This branch's `0.11.029`–`0.11.035`
are embedded as canonical ordinals in that array (note: `0.11.028` is absent — a
real ordinal gap to reconcile). Main introduced the drafts release model but never
landed its Rust side. Reconciling the two — turning `0.11.029`–`0.11.035` into
dependency-ordered drafts that fresh test DBs can still apply — is coupled to the
release cut and to step 4 above; do not invent a second durable migration ledger,
and do not rewrite these ordinals independently of the controller deletion.

- Review route and pending attention are now separate without another table:
  `attention_kind/work` stays for the interactive flow interval while
  `attention_at` clears after the routed Steer. A later terminal child Turn
  re-arms it transactionally and allocates one idempotent `evidence` revision
  on the open parent Epoch. Launch surfaces expose the nullable pending time so
  Swift does not mistake a parked User route for current attention.
- Wave now reserves one exact Run lease for a listener life, clears inherited
  Run/Session authority before spawning its resident, and drains direct input,
  oldest child Review, then other child evidence. Live and seed-only tests pin
  interruption of the repurposed background body, saved playhead, and no
  duplicate delivery of one unanswered child turn. Dirty canonical main no
  longer blocks the read-only resident.
- Wave provider captures now become product Run Launches using the resident's
  isolated process group. Recovery of a listener that dies after reserving but
  before its first Launch still belongs to the shared keeper transition; do
  not add a second Wave-specific recovery controller.
- The isolated full Rust integration run reaches the remaining controller
  split directly: `task_github_cache_tests::rest_failure...` constructs a live
  legacy Task process whose mirrored Run has no product Launch, so direct Run
  interrupt finds no boundary (`Query returned no rows`). Do not restore a
  Session interrupt fallback; the Session/body deletion must make every live
  writer a Run Launch.
- This pass now mints an independent opaque `LF_RUN_LEASE`, stores only its
  hash, resolves it directly to the exact active Run, clears it across distinct
  child launches, and fails closed when an agent context loses it. The old
  Session lease remains only for compatibility row writes; product controls no
  longer reconstruct authority from it.
- Observed root assistant output now persists on Turn, and the Project runner
  drains direct input then the oldest child Review before background work. It
  live-delivers to the active parent Turn when possible; otherwise it
  interrupts once, seeds the durable child projection, and leaves the
  background playhead on its next unfinished step. Wave scheduling still needs
  the same lane before the cut is complete.
- Review found that route and pending attention cannot be one flag. Parent
  Steer must clear only the answered turn while leaving Review open; the
  child's next terminal Turn must re-arm pending attention exactly once. That
  re-arm must also allocate a parent evidence revision so stale completion and
  old boundaries lose.
- Review also fixed live delivery advancing the background playhead: a Turn
  repurposed for child control now closes the active flow body as Interrupted
  even when the provider reports success. Deterministic live/seed-only harness
  tests still need to prove this end to end.
- The trace-only `LF_RUN_ID` environment variable is renamed `LF_TRACE_ID`, so
  the public Run vocabulary no longer collides with diagnostic lineage.
- `root_output` was folded into unpublished migration 0.11.031 rather than
  adding another intermediate ordinal. The complete 0.11.030-0.11.035 branch
  tail still needs conversion to dependency-ordered drafts at the final schema
  cut; the missing runtime `DRAFTS` contract remains unresolved.
- Stored Review/Handoff and ChildCommand are gone. The remaining core cut is
  the Wave control lane and deletion of the Project/Task Session-body
  controller.
- Main's account-lease broker confirms the capability semantics: resolve once,
  inherit one fixed opaque grant, prevent nested widening, and fail closed.
  Run lease validation stays local to SQLite; it does not need another SSH
  broker.
- Main's draft scripts/docs refer to a Rust `DRAFTS` registry that was not
  landed. The six unpublished architecture migrations must become drafts, but
  fresh test databases still need one coherent way to apply them before the
  release cut. Do not invent a second durable migration ledger.
- The branch has already exceeded the normalized 12,000-line deletion target.
  Restore focused CI/control behavior tests even if the physical count rises.

## Implementation review findings

- The first exact-lease draft lacked a uniqueness constraint on active lease
  hashes. The unpublished spine now enforces it, so one capability cannot
  ambiguously resolve to two Runs even if token generation is ever replaced.
- The shared Launch DTO exposed `OffsetDateTime` in Rust's structural serde
  shape while the pinned Rust/Swift fixture uses RFC 3339. The wire now names
  RFC 3339 explicitly and the cross-language fixture passes.
- Stable Task identifier rebind initially rewrote terminal Session history.
  The compatibility write now follows only the open Epoch; historical Sessions
  retain the identifier they actually ran under.
- Adding the stable Wave Epoch made legacy Wave deletion fail its foreign-key
  fence. Deletion now removes the Wave Epoch and its cascading facts in the
  same transaction; Runs or child Work still prevent unsafe deletion.
- Persisting every assistant delta makes partial output crash-visible, but it
  also writes the growing Turn output repeatedly. Keep this until the parent
  scheduling proof exists; a later batching change must preserve partial
  failure/interruption evidence rather than optimizing it away.
- The CLI Wave-resolution registry still named the deleted `reviews catch-up`
  command. Its stale row is removed rather than recreating the Review surface.

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
