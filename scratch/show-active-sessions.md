# W2-176 — Show Active Sessions without turning every process into a control

Mac Surface UX. Build the first user-visible **Control** destination and its
first view, **Active Sessions**, from the already-merged shared attention
projection (W2-123 / `lf roadmap`) and interactive attach contract (W2-175 /
`lf handoff`). Chat stays the default Wave experience; Control takes a deliberate
action to reach. Non-interactive bodies are visible and view-only; only
deliberate interactive handoffs expose Open/Attach.

## User-visible outcome

From a Wave header, a deliberate action opens Control. Its first view, Active
Sessions, is a machine-wide census grouped by Wave: every live Wave, Project
Session, Task Session, direct execution body, and interactive handoff, each with
stable identity, parentage, provider/model, Home, worktree, current step, age,
progress freshness, reason, and next owner **when the contract carries them**.
Ordinary non-interactive bodies are view-only — no attach, steer, interrupt, or
stop. Only an interactive handoff exposes **Open**, which attaches the exact
durable Session in embedded Ghostty. A handoff waiting on the human paints its
Task, Project, and Wave red even while its body is alive. Missing, stale,
unreachable, stopped, and unavailable evidence stay distinguishable from a
healthy empty state.

## Source of truth

The census is a **projection over existing Rust-owned contracts**, indexed by the
ids each contract already states. Swift performs no filesystem inference and
reconstructs no parentage — it reads the parent each row declares.

| Row kind | Source (`lf … --json`) | Contract type |
|---|---|---|
| Wave | `lf roadmap` (`WaveRoadmap.wave`) | `WaveSnapshot` |
| Project Session | `lf roadmap` (`RoadmapProject.runtime`) | `ProjectRuntimeSnapshot` |
| Task Session | `lf roadmap` (`RoadmapTask.runtime`, `.attention`) | `TaskRuntimeSnapshot`, `TaskAttentionSnapshot` |
| Direct execution body | `lf runs` (active rows) | `SkillRunEntry` |
| Interactive handoff | **`lf handoff list` (new)** | `InteractiveHandoffListRow` (new) |

The one gap: `lf handoff` exposes open/status/attach/complete/back/fail but **no
machine-wide list**, though `Store::list_interactive_handoffs(None)` already
exists. Enumerating handoffs for the census is impossible without it, so this
Task completes that contract with a thin, read-only `lf handoff list`. No new
lifecycle logic — it surfaces the store method that already ships.

## Affected surfaces and consumers

**New Rust (PR 1 — contract completion):**
- `HandoffCommand::List { parent: Option<String>, json: bool }` in `lf/mod.rs`;
  `handoff.rs` calls `store.list_interactive_handoffs(parent)` and prints
  `Vec<InteractiveHandoffListRow>`.
- `InteractiveHandoffListRow` DTO (in `interactive_handoff.rs`): the census
  fields — `session_id`, `parent_kind`, `parent_id`, `wave_id`, `status`,
  `provider`, `provider_session_id`, `home` (address string), `cwd`, `reason`,
  `created_at`, `updated_at`, `age_secs`. It carries **no** `argv`/`environment`:
  Open re-fetches the attach descriptor via `lf handoff attach`, preserving the
  W2-175 "attach records first-attach evidence" contract. Built from
  `InteractiveHandoff` via a `list_row()` accessor.
- Fixture `tests/fixtures/dto/interactive_handoff_list.json` + inline Rust
  round-trip test (mirrors the existing `interactive_handoff_attach` pattern).
- `docs/lf.md` handoff table gains the `list` verb.

**New Swift models (shared `Loopflow` module, testable):**
- `InteractiveHandoffListRow` in `Models/InteractiveHandoff.swift` — Codable
  mirror; round-trip in `DTOFixtureTests`.
- `Models/ActiveSessionsCensus.swift` — the pure projection. Input: a
  `RoadmapSnapshot`, `[SkillRunEntry]`, `[InteractiveHandoffListRow]`. Output:
  `[ActiveSessionWaveGroup]` → typed `ActiveSessionRow`s (Wave / Project / Task /
  direct-execution / handoff) carrying identity, parentage, provider/model, Home,
  worktree, step, age, freshness, reason, next owner, an `EvidenceState`, an
  `AttentionTint` (green/red/black/none), and the row's allowed actions
  (`[]` for everything except a live handoff, which allows `.open`). This file
  owns red propagation, evidence classification, and control derivation, so tests
  exercise them without a view.
- `RegistryQuery.activeHandoffs()` → `lf handoff list --json`, and
  `RegistryQuery.attachHandoff(sessionId:)` → `lf handoff attach <id> --json`
  (returns the existing `InteractiveHandoffAttach`).

**New Swift views (`LoopflowMac/Views`, dedicated files):**
- `ControlView.swift` — the Control destination shell: a segment/tab list with
  **Active Sessions** first and **Run History** as a quiet disabled affordance
  ("Coming soon"), no logic behind it.
- `ActiveSessionsView.swift` — renders the census groups and rows; Open button
  on handoff rows only; embeds Ghostty via the returned attach argv (same path as
  `TaskWorkspaceView`'s `GhosttyTerminalView(argv:…)`).

**Narrow shell edit (shared, minimal — W2-178 owns hierarchy):**
- `WaveDetailPane.header` gains one **Control** button (before the close button)
  and one `.sheet` presenting `ControlView`. Chat remains the default body; the
  sheet is the only added state. Nothing else in the pane changes.

**Not touched / excluded:**
- No Swift-owned Session lifecycle; Open is the only mutation and it delegates to
  `lf handoff attach`.
- No steer/stop/interrupt controls in this slice, even where the attention
  contract lists them.
- Run History is named only.
- No new Rust attention or runtime logic; roadmap/runs stay as-is.

## Projection rules (owned by `ActiveSessionsCensus`)

- **Liveness filter.** A row is "active" when: Project/Task runtime present with
  status in {created, starting, running, waiting, blocked} (not
  completed/abandoned/failed-terminal) — but a *failed/stopped* body still shows
  as a distinct non-healthy row, never dropped silently. Direct execution:
  `ended == nil` and status in {running, pending}. Handoff: status in
  {waiting, attached}.
- **Red propagation.** A handoff with status `waiting` (its parent chain =
  declared `parent_kind`/`parent_id` + `wave_id`) tints its Task, its Project,
  and its Wave red, overriding a green body. Implemented by indexing handoffs by
  parent id and walking the already-formed roadmap tree — no inference.
- **Evidence states** (`EvidenceState`, never defaults to healthy):
  - `unavailable` — `WorkEvidence.unavailable(reason)` for a Wave's projects;
    surface the reason.
  - `stale` — `evidence_age_secs`/`status_at` older than a threshold (start at
    120s; a knob, documented).
  - `stopped` — runtime present, `process_alive == false`, non-terminal status.
  - `unreachable` — the row's Home is remote (`WaveSnapshot.home.location ==
    ssh`) and the Wave is `live == false`.
  - `missing` — a Wave with zero census rows AND no unavailable evidence renders
    as an explicit "no active bodies" state distinct from an error, but a Wave
    whose evidence is unavailable never collapses to that.
  - `observed` — fresh, alive.
- **When-available fields.** `model` and `current step` are absent from
  Project/Task runtime snapshots; render them only when a source carries them
  (`SkillRunEntry.model`; project `iteration` as step for Project rows). Never
  invent — matches the run-ledger-owner learning in wave memory.

## Absent and error states

- `lf handoff list` fails / times out → Active Sessions shows the other census
  rows and a scoped "handoffs unavailable" notice; it does not blank the view or
  fake an empty handoff set.
- `lf roadmap` projects `unavailable` for a Wave → that Wave shows the
  unavailable reason, not an empty healthy Wave.
- No active bodies anywhere → explicit "No active sessions" empty state, visually
  distinct from any unavailable/error row.
- A handoff whose parent Task is absent from roadmap (race) → still listed under
  its `wave_id` as an orphan handoff row, red if waiting; never dropped.

## End-to-end proof

1. **Shared fixture render.** A new fixture drives one census containing every
   row kind — Wave, Project, Task, direct-execution, waiting handoff, remote
   Home, dead (stopped) body, stale-evidence row. `ActiveSessionsCensusTests`
   asserts: row kinds and counts; red propagation from the handoff up to its
   Wave; each `EvidenceState` mapping; that only the live handoff row exposes
   `.open` and every other row exposes `[]`.
2. **Real Product Wave census agrees with CLI.** On this machine,
   `ActiveSessionsCensus` built from live `lf roadmap` + `lf runs` +
   `lf handoff list` matches the identities and counts those commands print
   (spot-checked against `lf roadmap`/`lf runs` output).
3. **Interactive Open attaches the exact Session.** Open on a handoff row calls
   `lf handoff attach <session_id> --json` and launches Ghostty with the returned
   `argv`; the attached Session id equals the row's `session_id`. Non-interactive
   rows expose no attach/steer/stop/interrupt.
4. **VoiceOver.** Accessibility label functions (unit-tested) speak ownership
   (next owner), reason, freshness, and the available action per row.
5. **Gates.** `cargo test -p loopflow interactive_handoff`,
   `cargo test -p loopflow --lib` (handoff list command parse),
   `swift test --package-path swift` (census + DTO fixture + accessibility).

## Operational boundary

Census reads are daemonless `lf … --json` subprocesses off the main actor (the
existing RegistryQuery contract); the bundled daemon never gates them. Four
subprocess reads per Control refresh (`roadmap`, `runs`, `handoff list`, plus the
already-running `ls`), on the same 30s-ish cadence the Wave detail already polls.
Open performs exactly one `lf handoff attach` per click; attach is replay-safe.

## PR plan (serial, one Task worktree)

All three slices land on this one serial branch — no PR has been opened or
merged yet (the branch binary can't reach the registry to rotate serial PRs;
the orchestration surface lands it). The commits below are local and green.

- **Slice 1 — committed, green (local).** `lf handoff list [--active] [--parent]
  --json` + `InteractiveHandoffListRow` DTO + `interactive_handoff_list.json`
  fixture + Rust/Swift round-trips + `RegistryQuery.activeHandoffs()` /
  `attachHandoff()` + docs. Verified: `cargo test -p loopflow --lib handoff`
  (18 passed), `swift test --filter DTOFixtureTests`. Ships the missing
  enumeration contract; `activeHandoffs()`/`attachHandoff()` came forward with
  the decode since they are its natural consumers.
- **Slice 2 — committed, green (local).** `ActiveSessionsCensus` model (pure
  projection: red propagation, evidence classification, view-only control
  derivation) + `active_sessions_census.json` mixed fixture + census tests.
  Registry-independent, `swift test`-only.
- **Slice 3 — committed, green (local).** `ControlView` + `ActiveSessionsView` +
  the narrow `WaveDetailPane` header Control button/sheet. Chat stays the default
  body; the sheet is the only added shell state so W2-178 keeps the hierarchy.

Order lets each slice verify independently; slice 1 unblocks the Swift decode,
slice 2 is pure/testable, slice 3 is the visible Control destination and stays a
narrow shell edit.
