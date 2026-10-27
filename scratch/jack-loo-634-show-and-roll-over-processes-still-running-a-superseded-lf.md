# LOO-634: show and roll over processes on a superseded lf

Status: Session rollover and the Graph-boundary slice implemented (PR #1729),
current with origin/main 4a85fb3a5a. Remaining: the live demo after install. Source: the Task definition and cbda52's `scratch/database-locked-5-whys.md`
(Why 5). Jack asked for it to ship without stopping for review.

## Problem

A promote reaches only new processes: every running lf keeps the executable it
started on until it exits. After the 2026-10-26 hotfix, 35 processes stayed on the
buggy 0.14.16, 60 on 0.14.15 (below the store floor, so they lost the store), 4 on
0.14.14, and Desktop's helper kept starting 0.14.16. lf couldn't list them; the agent
killed 17 by pid from transcript heuristics. The stale set becomes visible and
movable under the existing launch contract (below).

## Demo

Promote while Sessions and a Graph are live. `lf doctor` warns
`lf-binaries: 12 processes on lf-d49c57c (0.14.16); 1 on Loopflow.app lf (0.14.16)`.
`lf monitor rollover --dry-run` lists what would move and why the rest stays.
`lf monitor rollover` stops each idle, unviewed owner and resumes its Sessions on the
selected lf; `lf session list` shows the same conversations, now on the new binary.
A Graph that was live through the promote launches its next step on the selected
lf; its runner stays on the old binary until the Graph ends, listed as
"moves at its next step". The doctor line shrinks to the runners and what was
listed as staying (attached terminals, busy turns, Desktop-helper children).

## Shape

- **Inventory** (`lf/commands/rollover.rs`): unfinished `kind=lf` Process rows the OS
  still shows live (receipt evidence = `Live`), grouped by running executable
  (`OsProcess::executable`: `proc_pidpath` / `/proc/<pid>/exe`) and its `--version`.
  A binary is *selected* when it belongs to the installed artifact set
  (`installation::installed_artifacts`), by path or digest (Desktop's `/Applications`
  copy of a selected helper). It *can't open the store* when its release is below
  `migrations::store_reader_floor`. No receipt → nothing is behind.
- **Surfaces**: `lf doctor` check `lf-binaries` (warn on any superseded binary);
  `lf monitor ps` text appends a BINARIES section (ActivitySnapshot JSON DTO
  unchanged); `lf self install promote` prints the superseded set after promoting and
  names Desktop's bundled helper when it is among them.
- **Rollover** (`lf monitor rollover [--dry-run] [--json]`): a superseded Session
  execution owner (interactive Sessions and `session serve`/`connect` hosts alike)
  moves when it holds Sessions, no viewer is attached, `owner_waits_for_person`
  holds (the same rule `transport/idle.rs` uses to end detached Sessions), and every
  Session has saved provider history. It is SIGTERMed like the idle policy, then
  each Session resumes through `restore::resume` on the selected CLI path, under the
  `resume-all` lock. Everything else is listed with its reason. Exit non-zero when a
  resumable Session failed to come back.

## Risk findings (2026-10-27)

- LOO-226's promotion-drain classifier no longer exists in the tree; its live
  remainder is `owner_waits_for_person` + saved provider session. Rollover reuses
  that rule rather than adding a third classifier.
- #1708 (LOO-610) merged on main (142e096fea) and keeps `monitor` top-level, so
  `lf monitor rollover` beside `ps`/`prune` stands; the Task's "`lf ps`" is
  `lf monitor ps` (there is no top-level `ps`). #1656 is still open; nothing here
  touches its helper-activation path.
- The blanket-pinning premise was stale. LOO-502 (Jack Heart, 2026-10-17,
  Infrastructure memory "Upgrade continuity") removed ordinary executable pinning.
  `os_process::resolve_lf_binary` on the main Home ignores inherited `LF_BIN` and
  returns the install entry gate, which selects and verifies the installed artifact
  at each exec; `RunningGraph::spawn` (`graph.rs:736`) calls it per step. So a live
  Graph's *next* step already runs on the selected lf with no stop, while the
  runner (cursor in memory) and its current step keep their binary. Rollover never
  stops or relaunches a Graph — no resume, no replay. Branches that keep the old
  binary: Desktop-helper runners (`installation::desktop_helper()` → the bundled
  helper a CLI-only promote doesn't touch); private Homes and draft copy stores
  (explicit selection; their Processes live in other stores, so main-store rollover
  never sees them).
- Compatible vs incompatible: when the runner's own release is at or above the
  store floor, the boundary handoff works. Below the floor, the runner can't read
  the store at its next `process_mark` and fails there (schema checks remain per
  LOO-502); its history stays and the Task's `lf task run` is the supported retry.
  Rollover states that, it doesn't act on it.
- Existing proof: `running_provider_uses_updated_install_for_commands_hooks_and_graph`
  (`os_process/installation_tests.rs`, CI job `task-installation`) shows a Graph
  *started after* a switch runs steps on the replacement. Nothing yet proves a Graph
  live *across* the switch — that is this slice's test.
- Classification and stop are separated by every earlier resume (seconds each), so
  a person can start a turn or attach a terminal in between. Rollover re-checks the
  pid's live evidence and the stay rules right before each stop and lists a changed
  owner as staying (2026-10-27).

## Decisions

- Placement: `lf monitor rollover` (Jack's Task named #1708's placement; verified).
- Graph runners get a third disposition, `NextStep`: no stop, reported as moving at
  their next step; Desktop-helper and below-floor runners stay with the reason
  (2026-10-27, from LOO-502's accepted contract; no new policy choice).
- Desktop helper: reported after promote and in doctor, not updated; #1656 owns
  helper activation and the CLI-only set carries no app artifact.
- Attached terminals stay listed ("detach or close, then roll over again") rather
  than being cut off.

## Concept review (2026-10-27, headless)

One action (`rollover`), three concepts (running binary, selected install, store
floor), one per-owner Disposition; surfaces reuse doctor, `monitor ps` and promote.
Nothing to rename or merge.

## Graph steps at a boundary (implemented 2026-10-27)

- `rollover::disposition` checks `SqliteStore::runs_graph` first; a Graph runner
  resolves to `NextStep` (listed under "Moving at their next step") unless it runs
  a Desktop helper ("update Desktop") or is below the store floor ("fails at its
  next step and `lf task run` retries its route"). Nothing is signalled. Session
  owners keep the earlier rule, and re-checks before each stop reuse the same
  `disposition`. JSON gains `next_step`.
- Proof `graph_live_across_switch_runs_next_step_on_selected_install`
  (`os_process/installation_tests.rs`, registered in
  `scripts/test_task_installation.py`): the prior install is a real lf copy (trailing
  bytes give it its own digest); a `git` on PATH holds step 1's first git call
  (only processes carrying `LF_GRAPH_ID`) at a fifo while a compatible switch
  settles; the Graph finishes with one
  `graph_processes` row, two successful steps on distinct nodes, step 1 on the
  prior binary, step 2 on the replacement, the runner recorded on the prior binary
  and exiting 0 on its own. A script wrapper can't stand in for the prior CLI:
  `dispatch_default_cli` re-execs any non-selected lf into the selected one, so a
  wrapper that execs the real binary loops forever (first attempt hung).
- `lf monitor rollover` after_help names Graph runners (`docs/lf-reference.md`
  doesn't render after_help; no regen).

## Exclusions

- The switch receipt recording `app_was_running: false` while Loopflow.app ran
  (named in the Problem, not the Outcome) — separate Task if wanted.
- Stopping, resuming or relaunching a live Graph to move its runner.
- Changing private-Home or draft-copy binary selection.

## Remaining

- Demo against the live store after install (see Checks).

## Done when


- `lf doctor` names every live lf process off the selected install, by binary and
  version, and flags releases below the store floor; it is silent when none are.
- `lf monitor ps` shows BINARIES with selected/superseded per binary.
- `lf monitor rollover` moves idle unviewed Session owners onto the selected lf with
  their conversations intact and lists every other superseded process with a reason.
- A CLI-only promote prints the stale set, naming Desktop's helper when stale.
- A Graph live through a compatible switch runs its next step on the selected lf
  without a stop or replay (disposable-install test), and rollover reports it as
  moving at its next step; Desktop-helper and below-floor runners are listed as
  staying with the reason.

## Checks

- Gate: `cargo clippy --all-targets -p loopflow -- -D warnings`;
  `cargo test -p loopflow --lib -- rollover executable_is_the_running_file restore`;
  `cargo test -p loopflow --test integration reference_matches_the_compiled_cli`;
  `cargo test -p loopflow --test doctor_tests` (all with `LF_*` unset, `< /dev/null`);
  `uv run python scripts/test_task_installation.py --test
  graph_live_across_switch_runs_next_step_on_selected_install` (Docker; else CI
  `task-installation`).
- 2026-10-27, after rebase: clippy, the focused lib tests (11) and the CLI
  reference pass. 2026-10-26: branch `lf doctor` here showed
  `29 processes on Loopflow.app lf (0.14.16)` with floor 0.14.16 (correctly no flag).
- 2026-10-27, after compress: clippy and focused lib tests (13) pass.
- 2026-10-27 realign: no rerun; the 4a85fb3a5a merge left every rollover file unchanged.
- 2026-10-27 Graph slice: clippy, `cargo test --lib -- rollover` (4) and the CLI
  reference pass. Docker proof: first wrapper design hung (above, fixed); reruns of
  the fixed test never reached it because OrbStack's VM crashed during compile and
  OrbStack.app then left /Applications. Deferred to CI `task-installation`.
- Demo: rollover and the `ps` section reach the live store only once installed (a
  branch build re-execs the installed lf for ordinary commands; `lf doctor` doesn't).

## Delete — do not maintain

- `install.rs` private `proc_pidpath` extern → `OsProcess::executable` (done).
- `resume_all`'s inline resume body → shared `restore::resume` (done).
- Rollover's second rusqlite connection for the floor → `SqliteStore::reader_floor`;
  the duplicated per-row floor query → `migrations::applied_floor` (done).
- `stays_because` + second `owner_sessions` read → one `Disposition` (done).
- Filtering `restore::render`'s footer → per-Session `restore::describe` (done).
