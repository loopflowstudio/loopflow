# Chapters — the repository's planning clock

2026-09-27 · LOO-298 · Design for Cut H7. Not built. Consolidates the target
already stated in [docs/waves.md](../docs/waves.md#the-planning-model), the
complete design's "Repository Chapter rotation" and the planning-ancestry
validator in [docs/architecture-reference.md](../docs/architecture-reference.md#core-models-and-apis).
Jack's model, from the Task: "Repository has Chapters: a repo-wide clock, one
current, incremented for every Wave at once. Project = (Wave, Chapter), unique;
owns that Wave's Tasks, KRs, metric targets and the Flow its Tasks run by
default. No 'recommended Flow'."

## What exists today (observed)

- `wave_chapters(wave_id, chapter_id, project_id, current, receipt)`: one clock
  per Wave. `Chapter` (`work/chapter.rs:59`) carries `wave_id`, `project_id`,
  `predecessors`, `tasks`, `phase`. `store/sqlite/chapters.rs::save_chapter`
  clears `current` for one Wave only.
- `ops/chapter.rs::rotate` runs for one Wave: preview, refresh boundary,
  `ensure_successor` (a Linear Project per Wave), activate, per-Task
  move/abandon/historical with retry, archive predecessors, refresh the PM
  snapshot. Classification and frozen metric evidence are sound and stay.
- A Project's Flow is a *recommendation*: `ProjectFlowPlan.recommended`
  (`pm/mod.rs:67`), parsed from a `recommended:` line in Linear Project content
  and read by `task run` as a default when present. Swift mirrors it as optional.
- `projects` has `wave_id`, no Chapter, no Flow column. `check_architecture.py`
  reports `wave_chapters` as the one missing owner.
- `lf wave new-chapter` exists with `--dry-run`; `lf wave history` and
  `lf status --chapter` read per-Wave receipts, including `legacy-<project>` ids.

## Usage (proposed; docs/waves.md already reads this way)

```sh
lf wave new-chapter --chapter 2026-10 --plan chapter.json --dry-run --json
lf wave new-chapter --chapter 2026-10 --plan chapter.json --json
lf wave history --wave infrastructure --json      # this Wave's Project per Chapter
lf status infrastructure --chapter 2026-09 --json # a past plan, frozen evidence
lf task run INF-123                               # the Project's Flow
lf task run INF-123 --flow incident               # explicit override (H1/H3)
```

`chapter.json` has one plan per Wave, keyed by stable Wave id:
`{"plans": {"<wave-id>": {"flow": "feature", "metric_targets": [], "krs": []}}}`.
A Wave missing from the plan is an error, never a silent retirement; an empty
plan is explicit. The preview lists every Wave and every Task disposition.
Retry with the same `--chapter` id resumes from the recorded boundary.

Recovery: activation is one SQLite transaction, so a crash leaves either the
old Chapter current with every successor prepared, or the new one current.
External moves, cancellations and archival stay retryable per Task afterwards,
as today. Started unfinished Tasks keep identity, worktree, PR and invocation.

## Model

```text
chapters      id (Jack-chosen, e.g. 2026-10), repository, state
              (preparing | current | completed), created_at, activated_at,
              completed_at, receipt_json (dispositions, frozen evidence per Wave)
              UNIQUE(repository) WHERE state='current'

projects      + chapter_id  NOT NULL REFERENCES chapters(id)
              + flow        NOT NULL   -- the template Tasks run by default
              UNIQUE(wave_id, chapter_id)

tasks         project_id (unchanged)  ⇒  Project ⇒ Wave; Chapter through Project
```

- A Wave's current Project is `projects WHERE wave_id=? AND chapter_id=(current)`.
  A Wave with no work has an empty Project row, created at activation.
- `Chapter` (Rust) loses `wave_id`, `project_id`, `predecessors`; it gains
  `plans: BTreeMap<WaveId, ChapterPlan>` (successor Project id, predecessor
  Project id, frozen metrics, task dispositions). One receipt, one row.
- `ProjectFlowPlan.recommended: Option<String>` becomes `Project.flow: String`.
  Linear Project content carries `flow: <name>`; the PM sync reads it into
  `projects.flow`; an absent line is an error at preview, not a default.
- `ensure_flow_position(task, None)` reads `projects.flow` through the Task's
  Project. `--flow` overrides for that invocation only.
- Validators: Project's Wave and Chapter name the same repository; one current
  Chapter per repository; a Task's Project must be in the current Chapter to
  receive a new invocation (transfer moves it there first).

Deleted with this: `wave_chapters`, per-Wave `save_chapter(activate)`,
`Chapter.wave_id/project_id/predecessors`, `ProjectFlowPlan` and its
`recommended` parsing and Swift mirror, the `legacy-<project>` synthetic
Chapter ids in `read_chapter`, `move_chapter_task`'s same-Wave check as a
separate query (the Project row carries it).

## Rotation, one operation

1. **Preview** (`--dry-run`): resolve every Wave in the repository, snapshot
   membership, classify each open Task with the existing `disposition`
   (Move / Abandon / Historical / Unresolved). Any Unresolved blocks.
2. **Prepare**: one Linear Project per Wave for the new Chapter, including
   empty plans, through the existing `ensure_successor` per Wave. Write
   `chapters(state='preparing')` with the full receipt. If Wave membership
   changed since preview, refresh and re-preview.
3. **Activate**: one transaction: insert successor `projects` rows with
   `chapter_id` and `flow`, set the new Chapter `current`, set the old
   `completed`, move local Tasks with `TaskDisposition::Move` to their
   successor Project, record frozen predecessor metrics per Wave.
4. **Transfer** (retryable, phase `Transferring`): per Task, Linear move or
   cancel; archive predecessor Projects; refresh PM snapshots. New Tasks filed
   during rotation stop the pass for reconciliation, as today.
5. **Complete**: `completed_at`, phase `Complete`.

Retirement keeps its evidence checks: authored work, PRs, worker claims,
`tasks.started_at`. Missing Runs alone never prove untouched backlog.

## Migration of existing Homes

`wave_chapters` holds per-Wave clocks whose ids may not agree. The draft:
imports every distinct `chapter_id` as a completed `chapters` row keyed by
`(repository, chapter_id)`; points each `projects` row at its chapter through
`wave_chapters.project_id`; Projects with no receipt get a `chapters` row named
`legacy-<project-slug>` marked completed; `projects.flow` is filled from the PM
snapshot's `recommended`, else the Wave's configured default, else `feature`,
and the import report lists every inference. **The one current Chapter for the
repository cannot be inferred**: if the Waves' current ids agree, that id
becomes current; if they disagree, the migration stops and the report asks for
the mapping. Nothing mints a fictional all-Wave rotation.

## Done when

The complete design's Done when 4: two Waves including an empty plan,
concurrent Task start versus retirement, loss of a provider response, missing
Task/Project reads, external reassignment, retry after activation; one current
repository Chapter; preserved active identity; frozen predecessor evidence. Plus:
`lf task run` with no `--flow` runs the Project's Flow; `check_architecture.py`
reports no missing owner; `lf wave history` shows one Wave's Project per
Chapter; a Home whose Waves disagree on their current Chapter is refused with
the two ids named.

## Decisions for Jack

1. **Initial repository Chapter on existing Homes** when Waves disagree: name
   it in the migration input (`--chapter <id>` on first `new-chapter`), or map
   by hand in the import report. Recommended: the first `new-chapter` after
   upgrade names it, and until then reads stay per-Wave-historical.
2. **`projects.flow` source of truth**: the Linear Project content line
   (`flow: feature`) synced into SQLite, or SQLite only with Linear as display.
   Recommended: Linear carries it, as KRs and targets already do.
3. **Chapter id shape**: free text as today (`2026-10`), or generated. Recommended:
   as today; the id is a name people say.
