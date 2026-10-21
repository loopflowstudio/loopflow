# LOO-557: Files view stops feeding itself; Process receipts stay bounded

Status: implemented and compressed (2026-10-21), both slices; uncommitted. Remaining: gate (full suites) and the Desktop demo. Evidence: `scratch/receipt-storm-5whys.md`.

## Problem

On 2026-10-16 Desktop's Task Files view launched `lf task diff`/`lf task file`
about 10 times a second for seven hours. About 98% of those runs left an orphan
receipt in `~/.lf/runtime/exec-processes`; nothing removes a receipt whose
Process has no row. By 2026-10-21 there were 237k, and `lf monitor work --watch`
re-parsed all of them every 2 s, holding a core and contributing to Jack's
"database is locked" failures. Four defects compound. Each is fixed here:

1. The Files view treats every non-`.git` filesystem event as a reason to re-read,
   including `lf`'s own trace writes and git-ignored build output.
2. A dead, unrecorded receipt is kept forever.
3. Activity cost grows with receipt history, not with live work.
4. Nothing shows the receipt count.

## Demo

With a Task's Files tab open in Desktop:

```bash
for i in $(seq 20); do lf task diff LOO-557 --files --base parent --json >/dev/null; done
cargo build            # inside the Task checkout; target/ churns
lf disk                # "Process receipts  N" line
lf disk clean          # removes dead receipts that never got a row
```

Expected: the Files tab issues no `lf task diff`/`task file` for the loop or the
build. Saving a tracked file produces one refresh. `lf disk` shows the count, and
`lf disk clean` leaves only receipts of live Processes or of unfinished rows.
`lf monitor work --watch` stays near idle with 200k receipts on disk.

## Risk findings (2026-10-21)

- **What a read writes.** Probe: one `lf task diff LOO-557 --files --base parent --json`
  in this checkout modified only `.lf/journal/traces/<trace>/events.jsonl`, and
  nothing in the git dir. From Desktop each run mints a trace, so it also creates
  a directory. The loop's trigger is the journal. Nothing else is involved.
- **Both observers have the loop.** Local: `TaskFileObservation` (FSEvents over the
  whole checkout) → `TaskFilesStore.invalidate`, which skips only `.git/`
  (`swift/LoopflowMac/TaskFileDocument.swift:403`). Remote:
  `watch_task_files` (`rust/loopflow/src/ops/task/workspace.rs:59`) sends
  `changed: true` for any non-access event. Swift answers with `invalidate([""])`,
  which re-reads every open document and the comparison. The remote loop is wider.
- **Git already defines the view's universe.** The comparison lists untracked files
  with `ls-files --others --exclude-standard` (`ops/task.rs:3700`). The directory
  listing drops ignored paths with `git check-ignore --stdin -z`
  (`ops/task/directory.rs:128`). `lf` git-excludes `.lf/journal/` in every
  repository it journals (`ensure_journal_ignored`). So a change to an ignored
  path cannot change the changes list or the chooser. One rule covers both the
  journal and `target/`/`.build/` churn from agent builds. A build-churn refresh
  is not a loop, but under load it costs one `lf` launch per 150 ms. It may have
  fed the Oct 16 rate.
- **A dead receipt with no row proves nothing to anyone.** A receipt's only reader is
  identity evidence for its recorded Process (`recorded_process_evidence`,
  `journal/mod.rs:1091`). Only the Process itself writes its row. Once
  `process_evidence()` is affirmatively `Dead` (PID gone or start time differs)
  and the store definitively has no row by locator or id, no future row can
  claim it. A receipt whose row exists but never completed is different. It is
  that row's only proof of exit, so it is kept. This answers the 5-whys question
  "is unrecorded-exit evidence worth keeping?": yes when a row exists, and no
  when none does.
- **Activity can read receipts per row.** `process_evidence` (control readers)
  already reads one receipt by the row's `receipt_locator`
  (`SqliteStore::process_receipt_path`). `load_snapshot` (`lf/commands/top.rs:233`)
  instead lists and parses the whole directory. A per-row read makes Activity
  cost O(unfinished rows) and uses the same identity rule.
- **Automatic bound exists.** `disk::rotate_when_due` runs hourly in-process on
  session captures, under the rotation lock. `lf disk clean` already calls
  `prune_dead_process_receipts`. Neither path needs a new trigger.
- **Reproduced (2026-10-21).** On `b4d5446`'s `invalidate`, the Swift acceptance
  test saw 11 `--files` reads in 2 s with nothing changing. That was only after
  one more fix: `TaskFileObservation` built its root with `resolvingSymlinksInPath`,
  which strips `/private`, while FSEvents reports real paths. So no FSEvents path
  under `/var` or `/tmp` ever matched. The root now uses `realpath`, as metadata
  already did. This is why existing tests passed through vnode watches alone.

## Chosen approach

### Desktop and remote watch: only relevant paths re-read

- The source of a filesystem event drops git-ignored paths, judged by
  `git check-ignore --stdin -z` in the checkout. This is the oracle the directory
  listing already uses. Filtering happens where the filesystem is:
  - Local: `TaskFilesStore`'s debounced invalidation filters its accumulated
    paths through one `git check-ignore` call before any `lf` read. That is at
    most one git process per 150 ms window. git writes nothing, so it cannot
    feed back.
  - Remote: `watch_task_files` collects event paths relative to the checkout.
    After its existing 150 ms settle, it drops ignored paths with the same git
    call and sends a frame only when relevant paths remain.
- `TaskFilesFrame` gains `paths: [String]` (relative, `.git/...` for metadata as
  local observation reports it, `[""]` for a rescan or watcher overflow).
  `changed` is removed. A heartbeat is a frame with empty `paths`. Swift passes
  remote paths to the same `invalidate` that local events use. Remote stops
  re-reading every open document on every event. The `.git` comparison rule
  (HEAD, ORIG_HEAD, index, refs) stays in Swift `invalidate` alone. The remote
  forwards `.git/...` paths and does not judge them.
- Locally, an ignored path that is an open document still re-reads that document
  only. The comparison is not re-read. The remote drops ignored paths entirely,
  so a remote open document under an ignored path is not re-read on change.

### Receipts: dead and unrecorded means removable

- `prune_process_receipts_at` removes a receipt when its evidence is `Dead` and
  either its row is completed (today's rule) or a successful read finds no row by
  locator or id. Unknown evidence, Live evidence, a store read error, or an
  unfinished row keeps the receipt.
- `rotate_when_due` runs `prune_dead_process_receipts` after rotation in the
  same background thread. Its failure is a `warn!`, like rotation's.
- `lf disk` (footprint) reports `process_receipts`: a directory entry count,
  without parsing. The human view prints `Process receipts  N`.

### Activity: cost follows live work

- `load_snapshot` stops calling `read_process_receipts_at`. For each unfinished
  non-agent row it reads that row's receipt by locator. This is the
  `process_evidence` path, factored so Activity and control readers share one
  function. A read failure gives that row `Unknown`, which is today's meaning
  of an unavailable inventory.

## Alternatives considered

- **Hard-code `.lf/journal/` in Swift and Rust.** It is smaller, but it mirrors an
  `lf` internal into two layers and still re-reads on every build write. Rejected.
- **Move traces out of the checkout** (for example `~/.lf/journal/<repo>`). This
  removes the event for every watcher, including editors. It reshapes
  `context_usage`, trace locators and journal tests, and the Files view would
  still need the ignore rule for build churn. Excluded here. It can be a Task of
  its own.
- **Local observation via `lf task files --watch` too** (one observer
  implementation). It would delete the FSEvents/vnode `TaskFileObservation`, but
  it costs a long-lived `lf` per Files tab and loses direct vnode watches for
  documents on volumes FSEvents excludes. Rejected for this Task.
- **Age/count cap on receipts.** An age cap would delete the only exit proof of
  unfinished rows. A count cap is arbitrary. The evidence rule is exact. Rejected.
- **Aggregate "N Processes unrecorded" warning.** Useful, but outside the
  accepted outcome. Excluded.

## Contract

- **Source of truth:** git ignore rules for the view's universe. The `processes`
  table plus OS identity for receipt liveness. Receipts remain identity evidence
  only.
- **Consumers:** `TaskFilesStore` (local and remote), `lf task files --watch`
  frames (Rust `TaskFilesFrame`, Swift `TaskFilesFrame`, the
  `tests/fixtures/dto/` fixture if one covers it, or a new one), `lf monitor work`
  Activity, `lf ps`/`lf top`, `lf disk`, `lf disk clean`, automatic rotation.
  No Swift reader of `lf disk --json` exists.
- **Absent and error states:** If `git check-ignore` fails (no repository, git
  missing), the batch is treated as relevant, which is today's behavior. The
  failure is logged once per store. A failed store open or read during prune
  keeps every receipt. A failed receipt read in Activity gives that row `Unknown`.
  A missing receipt directory counts as 0.
- **Operational:** Files tab idle with no relevant change issues no `lf` reads,
  and at most one `git check-ignore` per 150 ms while ignored paths churn.
  Activity snapshot time is independent of receipt-directory size. Prune is
  O(receipts) and runs only hourly or on `lf disk clean`.
- **Exclusions:** relocating traces; making read-only commands skip journaling;
  lock-holder tracing for "database is locked"; settling unfinished rows of
  dead Processes; the Oct 21 archive directory (left as is); the aggregate
  unrecorded warning.

## Delete — do not maintain

- `TaskFilesFrame.changed` (Rust and Swift) and Swift's remote
  `invalidate([""])` on `frame.changed`. Replaced by `paths`.
- `read_process_receipts_at` use in `top::load_snapshot`, and the
  `Option<&[ProcessReceipt]>` inventory parameter of `recorded_process_evidence`
  / `activity_state` / `collect_activity`. Replaced by the per-row read. Keep
  the full listing only for prune and `run_prune`.
- The "Unfinished or unrecorded Processes retain…" rule in
  `prune_process_receipts_at`. Unrecorded is no longer retained.

Forbidden: a second ignore matcher (for example the `ignore` crate in one layer
and git in the other), a hard-coded `.lf/journal` filter next to the git rule,
both `changed` and `paths` on the frame, an age-based receipt sweep beside the
evidence rule.

## Slices (one PR)

1. **This slice: Files relevance.** Write the Swift reproduction test first; it
   must fail on `b4d5446`. Then the local filter and frame `paths` (Rust
   watcher, Swift model, DTO fixture), deleting `changed`.
2. **Receipts and Activity (Rust).** Prune rule, rotation hook, `lf disk` count,
   per-row Activity reads, deleting the inventory parameter.

## Done when (gate runs once, headless)

- `swift test --package-path swift --filter TaskFilesTests`: a new test starts
  a real git repo with `.lf/journal/` excluded, opens the store with `observeFiles()`,
  and uses a `RegistryQuery` closure that counts `--files` calls. Each call
  writes `.lf/journal/traces/<uuid>/events.jsonl` and a file under an ignored
  `target/`. After a settle of about 2 s, the count stays at the initial read,
  where on `b4d5446` it climbs without bound. Writing a tracked file then re-reads,
  and the count settles. FSEvents may split one write into two batches, so the
  test does not require exactly one read. Existing `filesystemEventsRefreshDiffForFileNeverOpened`
  and `commitInLinkedWorktreeRereadsTheComparison` still pass. The remote-frame
  tests are updated to `paths`.
- `cargo test -p loopflow --lib journal::tests top`:
  `prune_removes_dead_receipts_unless_an_unfinished_row_needs_them` removes the
  completed+dead and no-row+dead receipts and keeps the unfinished-row+dead and
  live ones. `activity_reads_only_the_receipts_of_unfinished_rows` adds 10k junk
  receipts and makes the directory unlistable (mode 0o111); `load_snapshot`
  returns the same nodes, which proves it reads only locator paths.
- `cargo test -p loopflow --test integration dto_fixtures` covers the
  `TaskFilesFrame` fixture with `paths`.
- `cargo test -p loopflow --lib disk`: footprint JSON includes `process_receipts`.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean.

Check result (realign, 2026-10-21): no repair needed; reuses compress run — `swift test --filter 'TaskFilesTests|DTOFixtureTests'` 73 passed, focused `cargo test` lib/integration filters pass, fmt/clippy clean; full suites deferred to gate.

## Concept review (2026-10-21, headless)

The model holds. "What git ignores never moves the view" is one rule with one
oracle in both layers. A frame now names its paths, so remote and local use the
same `invalidate`. "Dead and unrecorded is removable" turns an unbounded history
into derived state. The receipt is kept only while it proves a row's exit.
Nothing needs to be merged or renamed.

Notes for compress. These are wording only, with no design change:
- `HomeUsage.process_receipts` (`disk.rs`) says "Live-Process receipt files". It
  counts every receipt file, dead ones included. Say "Process receipt files".
- In `rotate_when_due`, the prune comment says receipts are left "for every
  Activity reader". Activity no longer lists them. The remaining cost is
  `lf disk clean` and the directory itself.

An accepted consequence, recorded in `questions.md`: a remote open document under
an ignored path (`.env` is the common case) no longer refreshes on external
change. Before this change it did, because every event re-read every document. Local still refreshes it.
