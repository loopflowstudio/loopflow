# CLI ownership and readiness · LOO-338

Jack Heart requested every deferred CLI requirement on 2026-09-30. His latest
steer selects `--task` and `--wt` for location, `--wave` for context, removes
`--as` and `task run`, and waives the demo/review wait. The supervising session
owns shipment after implementation and focused proof. Earlier slice history is
retained at `6e1ba9fd2:scratch/implement-the-cli-owner-tree.md`.

## Accepted implementation

Main `de074a2eb` supplies the landed Exec/AgentSession/FlowSession, account and
Desktop navigation models. This work keeps those owners and released migration
bytes, with no parallel model or schema. CLI ownership and every baseline
keep/rename/merge/delete decision are in [the catalog](cli-command-catalog.md).
The [integrated surface](cli-integrated-surface.md) enumerates actual commands,
including additions from landed work, and all options.

- Task owns PR, wt, commit and sync. Ordinary branches need no registered Task.
  Selected Task PR/sync operations move executable, data and complete argv to
  the selected installation before local placement or effects.
- `--task` selects the Task checkout; `--wt` resolves the existing worktree.
  `--wave` enriches context without moving cwd and rejects another Task owner.
  Catalog and help inspection follow an explicitly selected checkout too.
- `flow start [template]` starts or continues the selected Task's managed Flow.
  Ordinary named Flows remain independent contributions. Restart explicitly
  replaces a managed graph; the hidden worker owns exact claim admission.
  These operations are distinct in the landed model and share its executor.
- Monitor list is the single Exec inventory. Active discovery/watch retains
  conversation linkage, missing observations and stdin-bound NDJSON. ps/top
  retain OS snapshots. Show resolves Exec or Session IDs; raw events/final
  belong to Session inputs. Usage retains accounting, missingness and historical
  filters; Work activity retains planning events. Replay remains explicit spend.
- Account has one live/default reader and explicit cached inspection. Landed
  per-provider bundles preserve selections through retries and background
  children; destination launch checks enforce access and restrictions. SSH's
  foreground lease cannot be stretched into detached authority.
- Repo owns planning connection, refresh and chapter rotation. `list wave`
  discovers authored nested/empty/unconnected goals without an execution DB;
  `wave list` reads durable placement; roadmap includes provider planning work.
  Wave detail is retained for its metrics, Project and Task-condition projection.
  Rename combines local placement/name and optional provider title.
- Arm prepares, requests auto-merge and returns. Land watches CI, repairs through
  ci-fix and re-arms until merged or blocked. Completion requires merged evidence.
  LOO-332 owns making Land finite once replacement repository ticks exist.
- A fresh Git project needs provider access, not Linear, for its first result.
  Guides use resolving short forms; generated reference/help retain owner paths.

## Delete — do not maintain

Removed parser/dispatch owners: root Exec/Runs, Task Run, Wave
Forget/Retire/Relocate, Wave Connect/Sync, duplicate User Name, Account Status,
Route Show, PR Status, Task Changes, Skill List/Show and Flow Validate.
Removed root `--as`, mode booleans, browser/diff boolean pairs, unused flags and
all registered aliases. Main's resident, daemon, chat, webhook, in-turn Flow
navigation and Chapter-table APIs remain deleted. Land's persistent watcher and repair fixtures remain until LOO-332 replaces them.

Compression removes the unreferenced `lf/commands/exec.rs` parser/dispatcher,
`Store::forget_wave` and its SQLite implementation and exclusive deletion checks,
and the unused synchronous PM rename wrapper. Monitor remains the sole command
inventory. Current-Wave filtering still proves that historical rows survive.
Wave placement checks now accept a Wave identity directly; launch-mode and
forwarded Flow argument selection each use one decision.

Preserve saved Flow graph/cursor/review, Task identity and PR/checkout, provider
selection restrictions, exact client signaling, real process ancestry, raw
provider evidence, DTO missingness and installation authority. No old spelling
receives a rejection shim: names freed from a definition namespace can be authored.

## Review findings resolved

The main merge exposed selected-Task routing loss and a nested runtime in Wave
rename. Forwarding now precedes placement; rename awaits the existing async
provider function. Root selector handling no longer drops Task attribution on
bare launch. Wave checks happen at the selected Task, including managed startup.
The supervisor restored Land
watch-and-repair and its acceptance test; the finite handoff belongs to Arm.
SSH nested-call detection now normalizes shorthand before inspecting the owner;
the moved Home namespace otherwise bypassed the existing transport boundary.
Repo Connect/Refresh each have one Wave selector; redundant leaf flags are gone.
Documentation ambiguity exposed competing sync/delete/publish/abandon leaves;
examples use the shortest resolving owner. No synonym aliases were added.

## Evidence and remaining checks

Live disposable walkthrough: Codex created the exact requested README in a new
Git repository, returned LOCAL_RESULT_CREATED and exited 0. Monitor, cached
Account, Home, Repo, Wave, Task, Session and Flow readers all exited 0 against
that real state. This proves a first local result without planning; it does not
claim live OAuth, remote-Home execution, rendered Desktop or a complete
release/merge lifecycle. Retained local artifact directory is recorded in
`/tmp/cli-live-path`; no credentials were copied into the repository.

Focused proof: Monitor state/prune preview (2), independent Task Flow contribution
(1), discovery/documentation (16+3), Account current/cached (1), mixed-provider
continuation (1), worktree/Wave selectors (2), earlier finite Land (1, superseded below), lease restrictions
(8), SSH boundary (16), Swift consumers (9), skill alignment (4), architecture
coverage and migration history passed. The disposable OS managed-review/start
proof passed (1), preserving its captured graph, cursor and exact pending review;
no provider or terminal was launched. Its earlier fixture assumed an unreserved
input and was corrected to compare the retained review input.

Final proof: prompt goldens (1), documentation (3), builtin contracts (14),
all-target Clippy, formatting, architecture, migration history and diff checks
passed. Main merged locally at `5091f9f0a`; implementation checkpoint `41b8e9d28`
contains the follow-up cull edits. The first continuation named remaining
tracked edits after recording the merge. After checkpointing, no continuation
receipt remained; `lf sync --manual de074a2ebd2cb54e6f7dde799400dae36c87bbbe`
then succeeded against the already-integrated target without changing HEAD.
The checkout is clean and the pinned main commit is an ancestor. At that checkpoint nothing was
pushed, published, landed or marked complete; shipment belonged to the supervisor.
Gate/CI retain affected-suite and release validation; these focused proofs are
not a hosted gate receipt. Current compiled counts: 127 commands below root, 428 flags, 84 positionals, 16 hidden commands and zero registered aliases.

## Supervisory corrections · 2026-09-30

Jack Heart requested three repairs, focused checks, a commit and a non-force push.
The message referenced a deletion list but contained no entries. Comparison with
main's LOO-298 deletion commits identified `rust/loopflow/tests/session_cutover_tests.rs`
and `tests/e2e/chapter_rotation.py`; both resurrected files are removed. The final
Session lifecycle suite remains; no replacement cutover test is introduced.

Land retains the automatic watch → ci-fix → re-arm lifecycle in both CLI and typed
Flow dispatch. Arm remains the finite request endpoint, including `-c` and `--next`,
so a repair never recursively enters Land. The earlier finite-Land proof above is
superseded. LOO-332 owns consolidation after finite repository ticks replace the
watcher; those ticks are not implemented by this Task.

C080 (`wave status`) is retained: current Projects/KRs, Task conditions, metrics
and Session history require no served Wave. Its stale resident/loop comments are
removed; the DTO stays intact. C086 (`wave place`) is retained for nonresident
work: `cron_authority` reads Wave placement and newly created Projects inherit it;
Tasks inherit their Project's Home. It neither starts a resident nor moves existing
child work. Help and catalog name these actual consumers. No served-Wave control
or resident-health field remains in either command.

Review found a second finite-Land dispatch in typed Flow execution and a recursive
repair instruction; both now use the original watcher/Arm split. Existing final-model
landing acceptance covers direct/Flow success, failed CI, blocked repair, queue state
and merged evidence, without recreating cutover coverage.

Supervisory checks: isolated `cargo test -p loopflow` (land_tests, status_tests, wave_repository_ownership, documented_commands, golden_prompt, and lib ops::pr_landing::tests) — 63 passed after correcting the restored Flow fixture to `--mode batch`; `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `git diff --check` — passed.
