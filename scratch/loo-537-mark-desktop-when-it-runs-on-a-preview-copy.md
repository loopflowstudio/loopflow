# LOO-537: Mark Desktop when it runs on a preview copy

Status: both slices implemented 2026-10-20 (uncommitted on the branch; realigned
against origin/main 1d11fd2372). Demo remains. Task asked for by Jack Heart 2026-10-20
as an autonomous follow-up to #1612.

## Problem

A build carrying unreleased migration drafts works on
`~/.lf/copies/<digest>/loopflow.db` (`store/copy.rs`; implicit by Jack's choice
2026-10-20). The CLI says so on stderr; Desktop says nothing. A dev-built
Desktop shows real-looking Tasks and Sessions whose writes never reach the
shared store, so work done there is easily mistaken for real work.

Second, smaller ask: first use of a new draft set took 8.6 min on a debug build
against an 8 GB store. Time it on a release build and record it in
`store/MIGRATIONS.md`.

Jack Heart, 2026-10-20: this is developer tooling; keep it light and never let
it reach normal users. Released builds carry no drafts, so `copy` is always
`null` and the capsule never renders there; keep it that way (no setting, no
copy text outside the capsule).

## Demo

1. Add a throwaway draft (`uv run python scripts/new_migration.py preview_probe`,
   body `CREATE TABLE preview_probe(id INTEGER);`), build the app bundle.
2. Open Desktop. The titlebar breadcrumbs end with a **Preview** capsule;
   hovering it shows `Preview copy: ~/.lf/copies/<digest> — work here is not in
   the shared store`.
3. A release-built app (no drafts) shows no capsule.

## Approach

Desktop already keeps one long-lived reader per window,
`lf monitor work --watch --json`, and every `WorkFrame` carries `home`. That
reader *is* the store read, so the fact rides there:

- **Rust** `lf/commands/work_watch.rs`: `WorkFrame` gains `copy: Option<String>`
  after `home` — the copy *directory*, `selected().parent()` (`selected()`
  returns `…/loopflow.db`), else `null`. Computed once in `run()` and bundled
  with `home` as a private `Origin` that `enqueue` and the heartbeat thread take.
- **Swift** `Loopflow/Services/WorkObservation.swift`: `WorkFrame.copy: String?`
  decoded with `decode(String?.self, forKey: .copy)` (key required, value
  nullable, like `answers`); add `copy` to `CodingKeys`, the memberwise `init`
  and `decode(line:)`. Every `WorkFrame(sequence:…)` call and inline frame line
  in the Swift tests gains `copy: nil` / `"copy":null`.
- **WorkModel**: `private(set) var previewCopy: String?` set from every applied
  frame (heartbeats included — the value is constant per reader). The data
  identity `workDataDirectory` becomes `frame.copy ?? frame.home`, computed once
  in `apply(_:)` and used by the heartbeat guard, change check and assignment: the copy is
  where the data lives, so a reader replaced across a shared↔copy switch drops
  Machine content and pending confirmations exactly as a home change does. All
  existing `workDataDirectory` uses (`confirmWorkUpdate(after:)`, the workflow
  catalog guard, directive/workflow/run confirmations) compare tokens and never
  reach a subprocess, so this is consistent.
- **View** `WorkBreadcrumbs` (`Views/WorkNavigator.swift:186`): its body is a
  `ViewThatFits` over two trails, so the capsule sits outside it in an `HStack`
  and never drops when the Wave crumb does. When `previewCopy` is set: a small
  `Preview` capsule, `.help(...)` naming the path with `~` abbreviation,
  accessibility id `preview-copy`. Rendered with the existing `WorkChip`
  (`.warning` tone, i.e. `warningTone`) rather than a new capsule view; no new
  token.

Why not other reads: `lf portfolio` reads a config file, not the store; `lf
doctor` runs only from the telemetry view; `lf machine id` is identity, and
the copy keeps the same Machine. The Work reader is the one read that is both
always open and actually reading the copied database.

### Timing (release build)

Build `cargo build --release -p loopflow` with the throwaway draft present, run
`time target/release/lf status` against the real default home (creates a copy —
local, removable, isolated from remotes by design), then remove
`~/.lf/copies/<digest>` and the draft. Break the time down with a second run
under `LF_LOG`/timestamps if over a minute: snapshot (expected ~0.02 s),
`apply_set`, draft SQL, `validate_foreign_keys`, `validate_persisted_json`.
Record total + split in the copy paragraph of `store/MIGRATIONS.md`. Recording
is the deliverable; speeding it up is not this Task.

## Risk findings

- `copy::selected()` is `pub(crate)`; `work_watch` is in the same crate. No
  visibility change. It hashes the draft manifest per call; call it once.
- Rust serde reads a missing `Option` as `None`, so the Rust DTO can't force the
  key (true of `answers` today too); Swift's `decode(T?.self)` does. The Swift
  missing-key check covers `copy`.
- `copy::enter()` runs in `lf.rs::main` before every command, so the
  `monitor work` process resolves the same copy as the app's other `lf` calls.
- Custom `LF_HOME` (lf-world, tests) never selects a copy, so fixtures and
  world-backed screenshots stay unmarked without extra handling.
- Remote Machines don't use `watchWork`; the marker describes the local lf only,
  which is what the app ships.
- First use on a copy blocks the reader's first frame for the migration time
  (minutes on debug). Out of scope; the timing record makes the cost visible.

## Contract

- **Source of truth:** `store::copy::selected()`. `WorkFrame.copy` and
  `WorkModel.previewCopy` derive from it.
- **Surfaces:** `WorkFrame` (Rust, Swift), `tests/fixtures/dto/work_frame.json`
  (`"copy": null` on every frame; frame 7 carries a path), the captured
  `task_lifecycle.json` frames (`"copy": null`), inline frame lines in Swift tests, Rust
  `dto_fixtures.rs` and Swift `DTOFixtureTests` assertions, `swift/README.md`
  Work-reader paragraph (one sentence), `MIGRATIONS.md` timing.
- **Absent state:** `null` = shared store. No frame yet = no capsule.
- **Exclusions:** no CLI flag, no menu-bar/Dock badge, no change to copy
  creation or its speed, no marker on remote Machines.
- **Forbidden:** inferring the copy from a `/copies/` path substring in Swift;
  a second read just to learn the flag; defaulting the DTO field.
- **Delete:** nothing; additive.

## Slices

1. **Done** — Rust field + fixture + Rust DTO test; Swift DTO + WorkModel +
   capsule + view test (`WorkModelStreamTests.previewCopyIsMarked`).
2. **Done** — Release-build timing recorded in the copy paragraph of
   `rust/loopflow/src/store/MIGRATIONS.md`: 265 s on 8.8 GB / 910k `processes`
   rows; the draft is ~0.1 s, `validate_foreign_keys` 49 s, and `copy::sanitize`'s
   per-row-triggered `processes` UPDATE 284 s (628k rows carry values, so a
   `WHERE` filter would barely help). Speeding it up is a follow-up, not this Task.

## Done when

- `cargo test -p loopflow --test integration dto_fixtures::work_frames_keep_each_part`
  passes with `copy` asserted null on one frame and a path on another.
- Swift `DTOFixtureTests.workFrame…` decodes `copy` and rejects a line missing it.
- A `WorkModelTests` case feeds a frame with `copy` through the existing fixture
  feed, then inspects `WorkBreadcrumbs(model:)`: `preview-copy` is present with
  the path in its help; a `copy: nil` frame shows none; a later frame whose
  `copy` differs drops Machine content as a home change does. Run via the
  headless `xcodebuild test` command in `swift/README.md`.
- `MIGRATIONS.md` names the measured release-build first-use time and the
  store size it was measured on (`~/.lf/loopflow.db` is 8.2 GB on 2026-10-20).
- Demo: dev-built app with a draft shows the capsule; a draft-free build does not.

## Checks

- Compress 2026-10-20: `cargo test -p loopflow --test integration dto_fixtures::work_frames_keep_each_part` + clippy pass; focused Swift `WorkModelStreamTests`+`DTOFixtureTests` xcodebuild test exit 0 (Rust `home`/`copy` bundled as `Origin`; Swift data identity computed once).
- Timing 2026-10-20: `cargo build --release -p loopflow` with throwaway `preview_probe` draft; `lf status` first use 265 s (split via temporary instrumentation, reverted); draft and copy removed.
- Compress 2026-10-20 (second pass): no reductions; slice 2 already recorded, no `preview_probe` draft left; `lf context` within budget. No rerun.
- Realign 2026-10-20: plan, code and `MIGRATIONS.md` agree; the remaining `~/.lf/copies/838775c8c5106610` is the live copy for main's committed `reader_floor.sql` draft, not the probe's, so it stays. No rerun.
