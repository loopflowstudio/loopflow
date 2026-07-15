# W2-166 — Resolve Cadenza Linear team ownership

Task Session `ts_cb980c1479b34d47b5e5f02205f08091`. One worktree, ordered serial PRs.

## What is actually broken (reproduced from code + config, 2026-07-14)

Cadenza's six waves each bind the **same** Cadenza Linear team and their own
Initiative in their repo's `GOAL.md` (`/Users/jack/src/cadenza/wave/*/GOAL.md`):

```
linear_team: 60558c53-2169-49f8-a76a-1f4586705aa9   # all six waves
linear_initiative: <one per wave>                    # core/ear/feedback/release/scores/theory
```

The binding has existed since the Asana→Linear migration (cadenza `85f329a`).
Yet the *Unified Practice Targets* Project and issue **W2-157** carry the `W2`
prefix — the shared **loopflow** team's key, not Cadenza's. `W2-*` is proof the
object was created against `config.linear.team` (the machine-global shared team),
never against `60558c53…`.

Three independent defects let this happen and kept it invisible:

1. **Creation can silently fall back to the shared team.**
   `resolve_team` (`rust/loopflow/src/ops/pm.rs:525`) returns the wave's
   `pm.linear_team`, *or else* `config.linear.team`, *or else* (via
   `LinearClient::resolve_team_id`, `pm/linear.rs:300`) the auto-created default
   `"Loopflow"` team. So any creation run where the wave binding is not visible —
   wave GOAL.md not yet bound at creation time, or a `cwd`/worktree whose
   `wave/<name>/GOAL.md` lacks the field (`read_wave_config` reads
   `repo/wave/<name>/GOAL.md` only, `engine/wave_config.rs:83`) — lands the
   Project/Issue on the shared team with a `W2` prefix. Fail-open, not fail-closed.

2. **`reteam` repairs Issues but never the Project.** `pm_reteam`
   (`ops/pm.rs:1456`) classifies and moves *issues* via
   `move_item_to_team` = `issueUpdate(teamId)`. There is **no**
   `projectUpdate(teamIds:)` anywhere — `UPDATE_PROJECT_MUTATION`
   (`pm/linear.rs:97`) sets only name/description/content. A Project stranded on
   the shared team is unrepairable by the shipped path.

3. **Reads and `doctor` are team-blind for Projects, and `doctor` is
   repo-scoped.** `LIST_INITIATIVE_PROJECTS_QUERY` (`pm/linear.rs:67`) selects
   `id/name/description/content/initiatives` — never `teams`. `PmProject` has no
   team field, so the SQLite snapshot, `lf pm show`, status, and roadmap all
   render a wrong-team Project as fine. `lf pm doctor` = `pm_sync(plan)` over
   `list_local_waves(repo)` (`ops/pm.rs:1593`) — it only checks issue prefixes
   (`ops/pm.rs:1712`) and only for waves **in the current repo**. Run from
   loopflow it never sees Cadenza; run from Cadenza it never checks the Project's
   team. Nothing surfaces or blocks the stranded state.

**Classification of the defect:** primarily *incomplete rollout* (objects
created against the shared team before/around the per-wave binding, never
reteamed) exposed by a *product-model gap* (team ownership is enforced only for
Issues, never for Projects, and only fail-open). Not a cross-repo *discovery*
bug in the binding read itself — the binding is correct in Cadenza's repo — but
`doctor`'s repo scope is why no one caught it.

## Ownership policy (decision)

**The authoritative persisted mapping is Wave → Linear team**, held in
`pm.linear_team` (a stable team **id**) in that wave's `GOAL.md`. Team key and
issue identifiers stay presentation; the id owns identity. Every CLI and Swift
view already derives from this — no new source of truth is introduced.

Two reconciliations with W2-155:

- **A team MAY be shared by the waves of one product.** Cadenza deliberately runs
  six waves on one Cadenza team, each with its own Initiative. W2-155's literal
  "one team per wave" is wrong for this shape; the invariant that matters is
  *every wave binds an explicit team, and no wave rides the shared-loopflow
  fallback*. `pm_sync`'s "waves share a team" line (`ops/pm.rs:1620`) must be
  demoted from a defect to allowed-when-same-repo (still flag a wave sharing a
  team with a wave in a **different** repo, which is the real hazard).
- **Repos are where waves live, not a binding surface.** We do not add a
  repo→team mapping. A Project/Issue inherits its team from the wave that owns it.

**The fix's spine: fail closed.** Creation must never invent or borrow a team.
If a wave has no `pm.linear_team`, `lf pm project create` / task creation
**errors** with the exact `lf pm init --wave <w> --team-key <KEY>` recovery,
instead of falling back to `config.linear.team`/default `"Loopflow"`. This is the
one change that makes "creating work through Loopflow cannot silently attach it
to the legacy W2 team" true. (The loopflow **product** wave is itself currently
unbound — `wave/product/GOAL.md` has no `linear_team` — so this fix requires
binding product to the existing shared team explicitly, adopting its id/key,
before the fallback is removed. Coupled, not optional.)

## User-visible outcome

A user opening Linear sees every Cadenza Project and open Task under the Cadenza
team `60558c53…` with the Cadenza prefix; loopflow work stays under its own
explicitly-bound team. Creating a Project or Task through Loopflow against an
unbound wave stops with an actionable error rather than silently attaching to the
shared team. `lf pm doctor` reports a Project on a foreign team and names the
repair command.

## End-to-end proof

1. **Clean create (fail-closed + correct team):** from a clean Cadenza checkout,
   `lf pm project create --wave core …` then create a Task. The Linear Project
   appears under `60558c53…`; the Task gets the Cadenza prefix; `lf pm
   sync/show/doctor` agree; roadmap/status links resolve to the same stable ids.
   Repeat against a wave with no `pm.linear_team` → command errors with the
   `lf pm init` recovery, creates nothing.
2. **Detect:** `lf pm doctor` (run in Cadenza) lists *Unified Practice Targets*
   as belonging to a team other than the wave's bound team, and lists W2-157 as a
   stranded open issue.
3. **Repair (idempotent):** `lf pm reteam --wave <w>` (dry-run) prints every
   planned Project-team move and Issue move; `--apply` moves the Project via
   `projectUpdate(teamIds:)` and the open, session-free Issues via
   `issueUpdate(teamId)`, records new identifiers, comments the old→new mapping,
   and refreshes the snapshot. Second `--apply` run is a **no-op** (Project
   already on team, issues already carry the prefix → `Already`).

## Source of truth

Wave → Linear team id in `GOAL.md` `pm.linear_team`. Linear names/keys/identifiers
are presentation. The SQLite snapshot is a read model; it gains a Project `teams`
projection only so doctor/reteam can compare — the binding itself never moves out
of GOAL.md.

## Affected surfaces and consumers

- **Rust `pm/linear.rs`:** add `projectUpdate(teamIds:)` mutation +
  `move_project_to_team`; extend `LIST_INITIATIVE_PROJECTS_QUERY` to select
  `teams { nodes { id key } }`; a `project_teams(project_id)` read for discovery.
- **Rust `ops/pm.rs`:** remove the `config.linear.team`/default fallback from the
  *creation* path (fail closed); keep it out of the *read* path (reads stay
  team-agnostic so unbound waves still sync existing issues). Extend `pm_reteam`
  to plan+apply a Project-team move ahead of issue moves. Extend `pm_sync`/doctor
  to flag a Project whose teams exclude the wave's bound team, and to demote
  same-repo team sharing from defect to allowed.
- **DTO:** `PmProject` gains team ids (wire DTO — add to `PmShowResult` fixture +
  Rust/Swift/Python fixture tests; no `#[serde(default)]`). `PmReteamResult`
  gains project-move records (not a `--json` wire DTO — internal, matching the
  existing note).
- **Swift:** `PmShowSnapshot`/`WaveProject` decode the new field; render is
  additive (team badge optional). Verify no decode break.
- **CLI text:** `print_pm_reteam_result` and doctor output include the Project move.
- **Docs:** `lf pm` README section on team ownership + reteam covering Projects.

## Absent and error states

- Wave has no `pm.linear_team` → creation errors with `lf pm init` recovery; no
  Linear side effect.
- Project's `teams` is empty or the team is inaccessible → doctor reports it,
  reteam refuses that Project with an exact diagnosis (no guess).
- Project attached to multiple teams (legacy + Cadenza) → reteam reports the set
  and, on apply, sets teamIds to exactly the wave's team (the move is a *set*, not
  an add), logged explicitly before the side effect.
- Any open Issue with a non-terminal Task Session → deferred (existing
  `classify_reteam_item` rule, unchanged). Completed issues stay historical.
- No command moves a partial hierarchy: the Project move and its issue moves are
  planned together and reported before any write; a failure mid-apply is
  restart-safe because every step is idempotent (Project-already-on-team and
  identifier-already-prefixed are both no-ops).

## Operational boundary

Discovery and dry-run are read-only (no mutation without `--apply`). Apply is
restart-safe and idempotent; it preserves stable Project/Issue UUIDs and live
Session/PR ownership (UUID survives Linear's renumber on move, as with the
shipped issue path). Every planned mutation prints before the first Linear write.

## Exclusions

- Do **not** move any live Cadenza data during this investigation/clarify phase.
- Do not rename unrelated teams, redesign Linear's hierarchy, or renumber
  completed history (`W2-N`→prefix-N). Completed issues stay as historical refs;
  shipped `W2-*` references are immutable.
- Not solving cross-repo `doctor` aggregation (one command seeing every repo's
  waves) — note it as the reason detection lagged; the fix is per-repo doctor +
  Project-team check, run where the waves live.

## PR sequence (serial, one branch each)

1. **Detection — MERGED (PR #907, squash `de27a7ba1`).** `PmProject` carries `team_ids:
   Option<Vec<String>>` resolved from the existing initiative-projects query
   (`teams { nodes { id } }`, no extra round trip; `Option` so older cached
   snapshots still decode, matching `PmItem.url`). `lf pm doctor` flags a Project
   whose resolved teams exclude the wave's bound team and names `lf pm reteam` as
   the repair (`project_off_team`, pure + unit-tested). Dropped the false-positive
   "waves share a team" diagnostic — a product may share one team across waves.
   Swift `PmProjectSnapshot` mirrors `team_ids` (optional). Mock-server + pure
   unit tests; no live Linear.
2. **Repair — `reteam` moves the Project ahead of its issues.** Add
   `projectUpdate(teamIds:)` / `move_project_to_team`, extend `pm_reteam` to plan
   and apply the Project-team move (set, not add) before the issue moves, extend
   `PmReteamResult` + CLI output. Idempotent (Project already on team → skip).
3. **Fail-closed creation + bind product.** Remove the
   `config.linear.team`/default-`"Loopflow"` fallback from the *creation* path so
   an unbound wave errors with the `lf pm init` recovery instead of attaching to
   the shared team; keep reads team-agnostic. Requires binding the product wave to
   the shared team explicitly first (one live team lookup).
4. **Live Cadenza inventory + dry-run + apply (executed from Cadenza checkout).**
   Read-only inventory of every Cadenza Initiative/Project/open Task/completed
   Task/active Session; dry-run; apply; prove second apply is a no-op. Operational,
   not a code PR — run from the Cadenza repo where the waves live.

## PR 2 build target (this pass) — reteam repairs the Project

Concrete, computable slice on top of merged PR 1.

- **Primitive (`pm/linear.rs`):** add `MOVE_PROJECT_TO_TEAM_MUTATION` =
  `projectUpdate(id, input: { teamIds: [$teamId] })` and
  `LinearClient::move_project_to_team(project_id, team_id)`. `teamIds` is a **set**
  (replaces the Project's teams with exactly the wave's team), not an add — this is
  what pulls a Project off the shared team. Linear does *not* renumber a Project on
  a team move (unlike issues), so the Project id and slug are stable; nothing to
  select back. Mock-server unit test asserts the mutation shape + variables.
- **Plan + apply (`ops/pm.rs::pm_reteam_async`):** before the existing issue loop,
  resolve the wave's team once (already have `team_id`), read each Project's
  `team_ids` from the snapshot (PR 1 populates them), and classify: a Project whose
  teams exclude `team_id` is a **move**; one already containing it is **already**
  (skip — idempotent). On `--apply`, call `move_project_to_team` and refresh. Dry
  run prints the planned Project move(s) with the current team set.
- **Result + CLI (`PmReteamResult`, `print_pm_reteam_result`):** add
  `project_moves: Vec<PmReteamProjectMove { id, name, from_teams, applied }>` (kept
  internal — `PmReteamResult` is *not* a `--json` wire DTO, per memory). CLI output
  lists Project moves ahead of issue moves.
- **Order:** Project move first, then its issues. Both idempotent, so a mid-apply
  crash re-runs to convergence.

**Error/absent states (this pass):** Project `team_ids == None` (older snapshot,
teams unresolved) → skip with a note, never guess. Empty `team_ids` → treat as a
move (belongs to no owned team). A Project on multiple teams including the wave's →
`already` (owned), leave it (the set-move is only for Projects *missing* the team;
narrowing a multi-team Project is out of scope for the automatic path — flag it in
doctor instead). Wave unbound (`read_team == None`) → `pm_reteam` already refuses
with the exact recovery; unchanged.

**Exclusions (this pass):** no fail-closed creation change, no product-wave
binding, no live Cadenza mutation — those are PRs 3–4. Pure code + mock-server
tests; no live Linear in the PR.

**Proof:** `cargo test -p loopflow` — a new `pm_reteam` mock test drives a Project
on a foreign team + one stranded issue through dry-run (plan lists both, no writes)
then `--apply` (project `projectUpdate` + issue `issueUpdate` fired, snapshot
refreshed), and a second `--apply` is a no-op (`already`). Plus the linear.rs
`move_project_to_team` mutation-shape test.

## Build/verify target for pursue

`cargo test -p loopflow` (pm unit tests against the mock server + reteam
classification), the DTO fixture round-trip for the extended `PmProject`
(Rust + `swift test --package-path swift --filter DTOFixtureTests`), and the
end-to-end proof scenarios above run against a Cadenza checkout in dry-run first.
