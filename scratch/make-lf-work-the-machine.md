# W2-144 — `lf roadmap`: the machine-wide view of current and available work

**Task:** W2-144 (Loopflow API). **Worktree:** this one. **Directive v2:** the
command is **`lf roadmap`**, not `lf work`. `lf status` stays execution/attention
only; `lf roadmap` owns durable on-disk PM intent and overlays shared live
evidence per item. No `lf status --all`, no `lf work` alias.

## The one-line boundary

| command | plane | answers | source |
|---|---|---|---|
| `lf status [wave]` | execution | what is running, is it healthy, who is waiting | Session registry + live probes, one wave |
| `lf roadmap [wave]` | intent | the whole open roadmap joined to live evidence — now / needs you / available / later | PM snapshot ⋈ Sessions, all waves |
| `lf pm …` | planning | raw plan read + mutation | Linear ⋈ SQLite |

`lf status` is per-wave and process-first. `lf roadmap` is machine-wide and
plan-first: it starts from the durable PM snapshot (every filed Task, started or
not) and overlays whatever live Session evidence exists. That is the one
difference that matters — `status` can't show an unstarted Task (no Session
exists to report), and `roadmap` must.

## What already exists (do not rebuild)

`lf status <wave> --json` already produces the full join, per wave:
`WaveDetailSnapshot { wave, loop_state, projects: [ProjectDetailSnapshot], runs, attention }`
where each `ProjectDetailSnapshot` carries `project` (PM) + `runtime` (Session) +
`next_move` + `tasks: [TaskDetailSnapshot]`, and each task carries `task` (PM) +
`runtime` + `next_move` + `prs` + `active_pr`. The PM↔Session join is done by id
in `snapshot_projects` (`waves.rs:640`). This is *exactly* the per-item row
`lf roadmap` needs. **W2-144 is the machine-wide envelope over it plus one
derived section lens — not a new data model.**

Unstarted-Task rendering is already correct: `snapshot_projects` emits every PM
item as a `TaskDetailSnapshot` with `runtime: None` and
`next_move = "Task is ready to start"`. Availability is precisely
`runtime == None && !completed`, computed from one read (W2-141's finding).

## The envelope DTO (new, additive)

```rust
// waves.rs — sibling to WaveDetailSnapshot
pub struct RoadmapSnapshot {
    pub generated_at: String,            // RFC3339, stamped once
    pub waves: Vec<WaveRoadmap>,
}
pub struct WaveRoadmap {
    pub wave: WaveSnapshot,              // reused verbatim
    pub projects: Evidence<ProjectDetailSnapshot>,  // reused; Evidence so a
                                         // wave with no local PM snapshot says
                                         // "unavailable", never empty
}
```

`ProjectDetailSnapshot` / `TaskDetailSnapshot` are reused unchanged **except**
one added derived field per row:

```rust
pub struct TaskDetailSnapshot   { …, pub section: RoadmapSection }
pub struct ProjectDetailSnapshot{ …, pub section: RoadmapSection }
```

### `RoadmapSection` — a lens, not a taxonomy

```rust
#[serde(rename_all = "snake_case")]
pub enum RoadmapSection { Now, NeedsAttention, Available, Later }
```

Four buckets answering **"where does attention go"** — coarser than and distinct
from a runtime-state taxonomy. Derived once in Rust and stamped on the wire so
**every consumer agrees without re-deriving a nine-arm match** (the W2-141
mandate). Swift decodes `section`; it never recomputes it.

Derivation from primitives already on the row (no new signals):

| section | predicate |
|---|---|
| **Now** | live Session, self-owned (`next_move.owner ∈ {Task, Project}`), process observably alive |
| **Needs attention** | `next_move.owner ∈ {Human, Review, Wave}`, **or** process gone while status claims running (the `Liveness::is_gone` audit finding) |
| **Available** | `runtime == None && !completed` — an unstarted, ready Task |
| **Later** | terminal/completed rows, and filed items a Session recorded as blocked/deferred that aren't waiting on a person |

### Coordination with W2-135 (the load-bearing decision)

W2-135's `BodyCategory` (Working / Stalled / Recovering / NeedsInput / Stopped /
Failed / Terminal / Unobservable) is the shared supervision taxonomy — but it is
**on an unmerged branch with no wire, fixture, Swift, or decoder integration**
(`323484bce`, explicitly deferred behind W2-134 rebase). Per this task's bounds,
the first slice stays **additive**: `RoadmapSection` is derived from the same
raw primitives W2-135's `observe()` uses (durable intent × liveness × ownership),
**not** a competing status enum. When W2-135 lands `BodyCategory` on the wire,
`section` derivation collapses to a fixed **map** from BodyCategory → section
(Working→Now, Stalled/NeedsInput/Failed→NeedsAttention, Terminal→Later, …), a
handful of lines, not a fork. That mapping is the integration point; nothing
about the envelope, sections, or consumers changes. This keeps one taxonomy in
the codebase, owned by W2-135, and `roadmap` as its coarse view lens.

## Latency: one tmux call, zero git calls

The real hazard is subprocess fan-out. Today `snapshot_task_runtime` /
`snapshot_project_runtime` each shell out to `tmux has-session` **per Session**,
and `task_pr_empty` runs `git is_clean` + `rev_parse` **per active PR**. Single
wave: fine. Machine-wide across N waves × M sessions: the exact N-Task fan-out
the task forbids.

Fix, both additive and a strict improvement to `lf status` too:

1. Add `tmux_live_sessions() -> Result<HashSet<String>>` (one
   `tmux list-sessions -F '#{session_name}'`). Build **once** per command.
2. Thread it as `TmuxLiveness { installed: bool, live: HashSet<String> }` into
   `snapshot_task_runtime` / `snapshot_project_runtime`; `process_alive` becomes
   a Set lookup, not a subprocess. `Liveness` (the `is_gone` observability gate)
   reads `installed`.
3. `lf roadmap` passes `empty: None` for PRs — emptiness is execution detail
   that belongs to `lf status`, so **no git subprocess runs** in roadmap at all.
   Roadmap needs PR phase + number, both already in `PrSnapshot`.

**Budget:** on the real ~3-wave / ~60-task workspace, one `lf roadmap` does
exactly **one tmux subprocess, zero git subprocesses**, and all planning/session
reads are local SQLite. Named target: **≤ 400 ms warm**, verified with `time`
before submit. No per-wave and no per-task subprocess fan-out.

## CLI

```
lf roadmap                # global: every wave, grouped Wave → Project → Task
lf roadmap --wave product # scoped projection of the same global result
lf roadmap --json         # the RoadmapSnapshot envelope
```

New top-level `Roadmap { wave: Option<String>, json: bool }` in `mod.rs`,
dispatched in `bin/lf.rs` to `waves::roadmap(wave.as_deref(), json)`. Human
render groups by Wave then Project, then prints the four sections (Now / Needs
attention / Available / Later); each row: identifier, title, rank, section,
progress age when known, concise reason, next owner, active PR. Unavailable
evidence prints "unavailable: <reason>", never blank.

## Swift + fixtures

- Mirror `RoadmapSnapshot` / `WaveRoadmap` / `RoadmapSection` in
  `swift/Loopflow/Models/WaveWorkMap.swift`; reuse the existing
  `WaveProjectWork` / `WaveTaskWork` decoders, adding the `section` key. No
  defaults, no `?? fallback` — `RoadmapSection` is required (DTO rule).
- Add `RegistryQuery.roadmap(wave: String?) async throws -> RoadmapSnapshot`
  (`lf roadmap [--wave] --json`), one subprocess for the whole machine —
  retires the per-wave `pm show` / `status` fan-out the Mac does today.
- Fixture `tests/fixtures/dto/roadmap_snapshot.json` covering ≥1 row per
  section; add to Rust `dto_fixtures` and Swift `DTOFixtureTests`. Round-trips
  through both without defaults. One shared fixture proves CLI and Mac classify
  identically.

## Docs

`rust/loopflow/src/lf/commands/README` (or the CLI reference the repo uses):
teach `lf status`, `lf roadmap`, `lf pm` by example, one promise each, no
overlap — status = "is it running / healthy", roadmap = "what's being worked on
and what could be", pm = "raw plan + mutation".

## Slices (serial PRs, one worktree)

1. **Envelope + lens + batching** — `RoadmapSnapshot`/`WaveRoadmap`/
   `RoadmapSection`, the extracted per-wave builder shared with `lf status`, the
   batched `TmuxLiveness`, `waves::roadmap`, CLI wiring, human render, Rust
   fixture + unit tests for section derivation. Demo: `lf roadmap` lists all
   open Tasks across Product/Infrastructure/Intelligence in one call; the six
   proof-row kinds render distinctly.
2. **Swift + fixture parity** — mirror types, `RegistryQuery.roadmap`, shared
   fixture test proving CLI/Mac agree. (Mac *surface* rendering is W2-131 /
   mac-surface-ux, not this Task — W2-144 provides the contract they consume.)
3. **Docs** — the three-command example table; land removes this scratch file.

## Deferred / not this Task

- Mac roadmap surface rendering → W2-131 / mac-surface-ux (consumes the DTO).
- `BodyCategory` wire integration → after W2-135 lands; `section` re-expresses as
  a map then.
- Backlog decay (18/26 filed-unstarted) → wave-level judgment, not a view
  predicate. Roadmap renders everything filed, no hidden staleness filter.
