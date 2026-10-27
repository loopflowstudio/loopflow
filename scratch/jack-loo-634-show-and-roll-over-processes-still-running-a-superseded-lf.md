# LOO-634: show and roll over processes on a superseded lf

Status: implemented and committed on this branch, current with origin/main
4a85fb3a5a (2026-10-27; that merge touched only Graph/workflow files). Source: the Task definition and cbda52's `scratch/database-locked-5-whys.md`
(Why 5). Jack asked for it to ship without stopping for review.

## Problem

A promote reaches only new processes: LOO-107 pins every running lf and its children
to their binary (`LF_BIN`). After the 2026-10-26 hotfix, 35 processes stayed on the
buggy 0.14.16, 60 on 0.14.15 (below the store floor, so they lost the store), 4 on
0.14.14, and Desktop's helper kept starting 0.14.16. lf couldn't list them; the agent
killed 17 by pid from transcript heuristics. Pinning stays. The stale set becomes
visible and movable.

## Demo

Promote while Sessions and a Graph are live. `lf doctor` warns
`lf-binaries: 12 processes on lf-d49c57c (0.14.16); 1 on Loopflow.app lf (0.14.16)`.
`lf monitor rollover --dry-run` lists what would move and why the rest stays.
`lf monitor rollover` stops each idle, unviewed owner and resumes its Sessions on the
selected lf; `lf session list` shows the same conversations, now on the new binary.
The doctor line shrinks to what was listed as staying (Graphs, attached terminals,
busy turns).

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
- Graph steps are pinned through `LF_BIN` (`os_process::resolve_lf_binary`), and the
  Graph process holds its cursor in memory. Stopping a Graph and relaunching its
  route replays effects; architecture forbids resuming a stopped Graph. Moving a Graph
  is therefore blocked on a product choice (see questions.md), not on code.
- Classification and stop are separated by every earlier resume (seconds each), so
  a person can start a turn or attach a terminal in between. Rollover re-checks the
  pid's live evidence and the stay rules right before each stop and lists a changed
  owner as staying (2026-10-27).

## Decisions

- Placement: `lf monitor rollover` (Jack's Task named #1708's placement; verified).
- Graph processes and their steps are listed as staying, not moved.
- Desktop helper: reported after promote and in doctor, not updated; #1656 owns
  helper activation and the CLI-only set carries no app artifact.
- Attached terminals stay listed ("detach or close, then roll over again") rather
  than being cut off.

## Exclusions

- The switch receipt recording `app_was_running: false` while Loopflow.app ran
  (named in the Problem, not the Outcome) — separate Task if wanted.
- Un-pinning Graph steps (LOO-107 doctrine).

## Remaining

- Demo against the live store after install (see Checks).

## Done when

- `lf doctor` names every live lf process off the selected install, by binary and
  version, and flags releases below the store floor; it is silent when none are.
- `lf monitor ps` shows BINARIES with selected/superseded per binary.
- `lf monitor rollover` moves idle unviewed Session owners onto the selected lf with
  their conversations intact and lists every other superseded process with a reason.
- A CLI-only promote prints the stale set, naming Desktop's helper when stale.

## Checks

- Gate: `cargo clippy --all-targets -p loopflow -- -D warnings`;
  `cargo test -p loopflow --lib -- rollover executable_is_the_running_file restore`;
  `cargo test -p loopflow --test integration reference_matches_the_compiled_cli`;
  `cargo test -p loopflow --test doctor_tests` (all with `LF_*` unset, `< /dev/null`).
- 2026-10-27, after rebase: clippy, the focused lib tests (11) and the CLI
  reference pass. 2026-10-26: branch `lf doctor` here showed
  `29 processes on Loopflow.app lf (0.14.16)` with floor 0.14.16 (correctly no flag).
- 2026-10-27, after compress: clippy and focused lib tests (13) pass.
- 2026-10-27 realign: no rerun; the 4a85fb3a5a merge left every rollover file unchanged.
- Demo: rollover and the `ps` section reach the live store only once installed (a
  branch build re-execs the installed lf for ordinary commands; `lf doctor` doesn't).

## Delete — do not maintain

- `install.rs` private `proc_pidpath` extern → `OsProcess::executable` (done).
- `resume_all`'s inline resume body → shared `restore::resume` (done).
- Rollover's second rusqlite connection for the floor → `SqliteStore::reader_floor`;
  the duplicated per-row floor query → `migrations::applied_floor` (done).
- `stays_because` + second `owner_sessions` read → one `Disposition` (done).
- Filtering `restore::render`'s footer → per-Session `restore::describe` (done).
