# W2-151 PR2+3 — the remaining ambient-Wave consumers

PR1 (#915, merged) shipped `resolve_managed_wave_name` in `wave_context` and
routed `lf status` + every `lf pm` arm through it. This branch finishes the
contract for **every other** consumer that acts on "the wave I'm inside", so the
one rule holds everywhere — not another partial pass.

## Computable design contract

- **User-visible outcome.** A human or agent inside a Wave/Project/Task process
  (`LF_WAVE_ID` set) runs `lf chat`, `lf radio pub/sub`, `lf memory show/log/add`,
  `lf home`, or `lf cron add` with no `--wave` and acts on the same durable Wave
  that `lf status` / `lf pm show` already resolve. A hand-set `LF_WAVE_ID=<name>`
  now resolves instead of dropping; a stale UUID fails loudly instead of looking
  like "no wave here".
- **Source of truth.** The shared SQLite registry row (`Wave`: id + name). The
  environment is only a pointer used to find that row, never identity. Every
  consumer derives the wave *name* from the row and keys its own view off it
  (chat/memory endpoint pointer + journal, radio channel, home `GOAL.md`, trace
  attribution).
- **End-to-end proof.** `lf memory show` (through `chat::resolve_target`) resolves
  the same wave as `status`/`pm show` across the seven-cell matrix; unit cells at
  `chat::resolve_target` and `radio::ambient_wave` for the two previously-silent
  failures. Commands: `cargo test -p loopflow --test wave_resolution_tests` and
  `cargo test -p loopflow --lib -- commands::chat:: commands::radio::
  engine::wave_context::`.
- **Operational boundary.** Resolution is one local `get_wave` /
  `get_wave_by_name` — daemon-less, no Linear on any read path. The UUID arm hops
  a scratch thread (context assembly is sync, sometimes inside a runtime); the
  hand-set-name arm touches no store.
- **Exclusions.** `LF_CHANNEL` semantics and sub-channel (`family_head`)
  derivation are untouched — only the `WaveId` arm is unified. `lf cron
  sync`/`remove` stay declarative (explicit wave, reconciling `GOAL.md`). No wire
  DTO fields; no Swift/iOS model change. The launcher still exports the UUID
  (correct); consumers were the bug.

## The one rule (unchanged, from PR1)

`--wave` wins → else `LF_WAVE_ID` as a durable **UUID** mapped to its registry
name → else `LF_WAVE_ID` as a hand-set **name** used directly → else
`NoContext`. A UUID the registry has never seen is a loud `StaleIdentity`, never
silently re-read as a name.

## What was still broken (the WaveId arm, five ways)

Every consumer below read `LF_WAVE_ID` through `resolve_ambient_channel`'s
`WaveId` arm and then `id.parse::<WaveId>()` + `get_wave`. Two failure modes,
shared by all of them:

- **hand-set name** (`LF_WAVE_ID=product`): `id.parse()` fails → silently
  dropped (chat/radio/memory publish to nobody; trace/home fall back to the
  worktree name).
- **stale UUID** (`LF_WAVE_ID=<uuid not in registry>`): `get_wave` misses →
  silently dropped, indistinguishable from "no wave here".

## Changes

- **`chat::resolve_target`** — the `WaveId` arm routes through
  `resolve_managed_wave_name`. Hand-set names now resolve; a stale UUID is a
  loud error (reads error, publishes no longer silently succeed-as-drop). The
  `LF_CHANNEL` arm is untouched. `memory` inherits the fix (it shares
  `resolve_target`).
- **`radio::ambient_wave`** — same routing; signature becomes
  `Result<Option<AmbientWave>>` so `StaleIdentity` surfaces loudly instead of
  collapsing into the no-subscriber drop. `ambient_channel` (used by
  `lf radio sub`) threads the `Result`. `NoContext` still drops with exit 0.
- **`wave_context::resolve_run_wave_name` / `wave_name_for_id`** — gain the
  hand-set-name fallback by delegating to the shared resolver. Feeds trace
  attribution (`journal`, `bin/lf`), `lf home` routing, and prompt context
  assembly (`resolve_ambient_channel_name`). Errors stay non-fatal (omit
  attribution), but a hand-set name now resolves.
- **`lf cron add`** — `--wave` becomes optional and resolves ambient like the
  PM arms (`NoContext` ⇒ "pass --wave"). `sync`/`remove` stay declarative
  (explicit wave, reconciling `GOAL.md`).

## Boundaries preserved

- **publish drop semantics**: `NoContext` still drops chat/radio/memory writes
  with exit 0 (publish to no subscriber). Only `StaleIdentity` is loud.
- **reads**: `chat --history/--follow`, `memory show/log` treat `NoContext` as
  an error and now surface `StaleIdentity` too.
- **cache-only**: resolution is one local `get_wave` / `get_wave_by_name`. No
  Linear on any read path; `lf pm show --no-sync` never touches the network.
- **no wire DTO changes**: resolution is upstream of every `--json` shape.

## Proof

- Resolver matrix (PR1, `wave_resolution_tests.rs`) already covers the seven
  environments at the core.
- New end-to-end: `lf memory show` resolves the same wave as `status`/`pm show`
  across UUID / hand-set-name / explicit-override / stale / no-context.
- New unit tests: `chat::resolve_target` and `radio::ambient_wave` for the
  hand-set-name (was dropped) and stale-UUID (was silent) cells;
  `resolve_run_wave_name` gains a hand-set-name test under the env lock.
