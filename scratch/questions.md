# Open questions / blockers

## Asana token expired (operational, blocks PM refresh)

`lf op ingest` fast path failed:

```
warning: failed to pull PM items for wave/Desktop: Stored asana token has expired. Run `lf op auth asana` again.
Error: no roadmap items in wave/Desktop/
```

The "no roadmap items" message is misleading — `wave/Desktop/` does contain
`1-embedded-terminal-build-driver.md` and `2-native-chat-ux.md`. The real
failure is the expired Asana token blocking the PM mirror refresh.

**Action needed (human):** run `lf op auth asana` to restore PM sync. Until
then, ingest/PM-backed flows operate on the stale local mirror only.

## Ingest decision (assumption, headless)

Per the ingest step's documented fallback ("if the pull fails, ingest warns and
falls back to the local `wave/<name>/` mirror"), I fell back to the local
mirror instead of failing.

State at ingest time:

- `scratch/Desktop-native-chat-ux.md` already exists, committed in
  `12945db5 lf commit: ingest`, with a valid claim
  (`claimed_by: f6684ab5-...`, `claimed_at: 2026-05-19T02:01:39Z`,
  `asana_id: 1214270115439574` matching `wave/Desktop/2-native-chat-ux.md`)
  and `status: in-progress`.
- Priority 1 (`embedded-terminal-build-driver`) outranks the selected
  priority-2 item, but every local signal says it is the actively-worked p1
  task under a separate claim: wave MEMORY records embedded-terminal
  implementation verified 2026-05-19, and recent commits
  (`6b300ab1 desktop: clean palette terminal exit files`) are
  embedded-terminal work. This is consistent with why the prior ingest
  selected native-chat-ux over it.

**Assumption:** ingest is already satisfied. I left
`scratch/Desktop-native-chat-ux.md` intact rather than clobbering an
in-progress claim or re-picking a higher-priority item that appears claimed by
active work. With Asana unreachable the claim on item 1 cannot be re-verified;
if it is in fact free, a human (or a post-`lf op auth asana` ingest) should
re-pick.

## Review-design pass (headless, 2026-05-19)

`review-design` is normally interactive — a human reshapes the kickoff
elaboration. Run headless, I acted as the design partner and made the calls a
knowledgeable reviewer would, anchored to the wave README/roadmap and verified
against the current tree (two parallel Explore passes over Swift + Rust).

Outcome: the **approach held** — no architectural redirection. The work was
grounding, matching the wave-memory learning that the high-value review move
here is "catching invented fields, not re-litigating the approach":

- Fixed drifted `file:line` refs throughout (macOS text view `:69,82`→`:85/:17`;
  iOS inline `:108`→`:112-114`; parser is `WaveSessionView.swift:691-753` in
  the **view layer**, not pre-existing in `LoopflowCore`; store fns at
  `sqlite.rs:784/:747`; replay path `SessionState.swift:327-351,452-462`).
- Corrected `afterSeq: 0` → `afterSeq: nil` (the actual reconnect/replay
  parameter; `replayCompletedLastSeq` drives the live promotion).
- Grounded `SessionSummaryDto`: `Session` carries `wave_run_id`, **not**
  `wave_id`; `wave_id`/`wave_name` derive via the join the usage query already
  does; `title`/`message_count` from the already-fetched events batch.

**Executive scope call (headless, no human to confirm):** history is
**per-wave for v1** — `wave_id` is a required query param and DTO field. No
`list_sessions_for_repo` store query exists; mirroring `list_sessions_for_wave`
exactly keeps M2 a pure read-over-existing-write path. Repo-wide/cross-wave
history is moved to an explicit v2 out-of-scope. If a human wants cross-wave
history in v1, this is the one decision to revisit; everything else is
verified-as-designed.
