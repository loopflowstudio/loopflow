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
- Land prepares, requests auto-merge and returns. It does not declare a merge or
  complete a Task. Scheduled/release observation retains its separate owner.
- A fresh Git project needs provider access, not Linear, for its first result.
  Guides use resolving short forms; generated reference/help retain owner paths.

## Delete — do not maintain

Removed parser/dispatch owners: root Exec/Runs, Task Run, PR Arm, Wave
Forget/Retire/Relocate, Wave Connect/Sync, duplicate User Name, Account Status,
Route Show, PR Status, Task Changes, Skill List/Show and Flow Validate.
Removed root `--as`, mode booleans, browser/diff boolean pairs, unused flags and
all registered aliases. Main's resident, daemon, chat, webhook, in-turn Flow
navigation and Chapter-table APIs remain deleted. The CLI's persistent landing
watch test and its exclusive repair fixtures are deleted with that behavior.

Preserve saved Flow graph/cursor/review, Task identity and PR/checkout, provider
selection restrictions, exact client signaling, real process ancestry, raw
provider evidence, DTO missingness and installation authority. No old spelling
receives a rejection shim: names freed from a definition namespace can be authored.

## Review findings resolved

The main merge exposed selected-Task routing loss and a nested runtime in Wave
rename. Forwarding now precedes placement; rename awaits the existing async
provider function. Root selector handling no longer drops Task attribution on
bare launch. Wave checks happen at the selected Task, including managed startup.
Land returns on handoff, and its obsolete waiting/repair test was removed.
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
continuation (1), worktree/Wave selectors (2), finite Land (1), lease restrictions
(8), SSH boundary (16), Swift consumers (9), skill alignment (4), architecture
coverage and migration history passed. The disposable OS managed-review/start
proof passed (1), preserving its captured graph, cursor and exact pending review;
no provider or terminal was launched. Its earlier fixture assumed an unreserved
input and was corrected to compare the retained review input.

Final proof: prompt goldens (1), documentation (3), builtin contracts (14),
all-target Clippy, formatting, architecture, migration history and diff checks
passed. Main merged locally at `5091f9f0a`; the finishing checkpoint contains
follow-up cull edits. The first continuation correctly named those remaining
tracked edits after recording the merge; it published nothing.
Gate/CI retain affected-suite and release validation; these focused proofs are
not a hosted gate receipt. Final compiled counts: 126 commands below root,
419 flags, 84 positionals, 16 hidden commands and zero registered aliases.
