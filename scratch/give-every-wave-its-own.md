# W2-155 — Give every Wave its own Linear team and Task prefix

## The one-sentence problem

A task's prefix (`W2-155`) is a pure function of the Linear **team** its issue
lives in — Linear derives the `identifier` from the team key and we copy it
verbatim into `PmItem.identifier`. Today **every wave shares one team**
(`config.linear.team`, a single machine-global `Option<String>`), so everything
is `W2-*`. Bind the team **per wave** and the prefixes fall out for free:
`PRD-*` for product, `INF-*` for infrastructure, `INT-*` for intelligence.

Nothing in loopflow generates `W2`. There is no prefix logic to write — only a
binding to move from global config into the wave.

## What's already true (and load-bearing)

- **Reads are team-agnostic.** `list_projects` fans from `initiative(id) →
  projects`; `list_items` from `project(id) → issues`. Neither touches the team.
  So binding a wave to a *new* team never breaks reading its *existing* `W2`
  issues — they still sync through Initiative → Project → Issue. This is what
  makes a forward-cut safe without touching a single existing issue.
- **The team primitive exists.** `LinearClient` already carries
  `team_id: Option<String>`; `resolve_team_id` (linear.rs:228) resolves it or
  creates a team via `CREATE_TEAM_MUTATION` (name + key); `team_key_from_name`
  derives a key. `create_project` and `create_item` both route through
  `resolve_team_id`. The client is already per-instance — it just gets fed the
  global team today.
- **Loopflow identity is already independent of the team key.** Wave selection
  is by wave name (`product`), project slug from the project *name*, Task
  Sessions keyed by stable issue **UUID** (`issue_id`), not identifier. The team
  key is Linear presentation only. The design's job is to *not regress* this.
- **Sessions survive a team move.** Linear preserves the issue UUID on a team
  move; Task/Project Session lookup is by UUID. Only the cached
  `issue_identifier` (session_context.rs `LinearIssueSnapshot`, the snapshot
  payload) goes stale and needs a re-sync.
- **`lf pm doctor` exists** — it's `PmCommand::Doctor` → `pm_sync(plan:true)`, a
  dry-run diff. Extend it; don't add a new command.

## The load-bearing tension: Linear renumbers on team move

**Linear does not preserve the issue number across a team move.** Moving `W2-155`
into a fresh `PRD` team yields `PRD-<next-in-PRD>` (e.g. `PRD-7`), **not**
`PRD-155`. The user story's "`PRD-123` because `W2-123`" mental model is not
achievable through a team move. A dry run cannot even predict the exact new
number — Linear assigns it at move time.

This forces the completed-history policy, and it's why the design below is a
**forward-cut with bounded, traceable migration**, not a rename-in-place.

## Design: forward-cut, then bounded migration

### Binding (PR 1)

Add a per-wave team binding, mirroring the Initiative binding exactly:

- `WavePmConfig` (wave_config.rs:17) gains `linear_team: Option<String>` — the
  stable Linear **team ID** (identity), written to `GOAL.md` frontmatter under
  `pm.linear_team`. The team **key** (`PRD`) is mutable presentation; we never
  store it as identity or derive anything from it.
- `write_initiative_to_goal` (ops/pm.rs:1613) also writes `pm.linear_team` when
  `pm init` resolves a team.
- `build_client` (ops/pm.rs:448) reads the **wave's** `pm.linear_team` instead
  of `config.linear.team`. `config.linear.team` degrades to a fallback default
  only when a wave has no binding (keeps every currently-unmigrated wave working
  on `W2` until it's initialized). `build_client` currently takes only
  `(repo, provider)` — thread the wave through, or resolve the team inside
  `resolve_context`/`resolve_wave` and pass it to `LinearClient::new`.

### `lf pm init --team-key <KEY>` (PR 1)

`pm init` currently links only the Initiative. Extend it to also create/adopt the
team:

- New `--team-key` (e.g. `PRD`) and optional `--team-name`. Default team name =
  title-cased wave; default key = derived from the wave name.
- Resolution: list teams; match by key first, then name.
  - Key exists **and** name matches → adopt (record ID).
  - Key exists with a **different** name → **conflict**: refuse with the exact
    recovery action ("`PRD` is taken by team X; pass `--team-key` or rename").
  - Absent → create via `CREATE_TEAM_MUTATION` (name + key).
- Write `pm.linear_team: <id>` to GOAL.md; commit alongside the Initiative
  binding (same commit path `pm init` already uses).
- **Idempotent**: a second `pm init` with the binding present is a no-op (matches
  the existing Initiative-already-linked early return at ops/pm.rs:822).

Demoable win for PR 1: `lf pm init --wave product --team-key PRD`, then
`lf pm task create` under a product Project → the new issue is `PRD-N`.
Repeat for infrastructure (`INF`) and intelligence (`INT`). Existing `W2` issues
keep reading unchanged.

### Migration (PR 2): `lf pm reteam` (dry-run default)

An idempotent, restart-safe command that moves a wave's **settled** open issues
from the old shared team into the wave's bound team.

- New Linear op `move_item_to_team(item_id, team_id)` — one `issueUpdate(input: {
  teamId })` mutation returning the **new** `identifier` (linear.rs has the
  `issueUpdate` shape already; add a variant that selects `identifier` in the
  response). No such op exists today.
- **Dry-run is the default.** It names every issue that will move (old
  identifier), every one it will **defer** (with reason), and the target team —
  with the explicit caveat that the new number is Linear-assigned at move time,
  not predictable.
- **Protect live/in-review work.** Before moving an issue, look up its Task
  Session by stable UUID. Any **non-terminal** Session (running, submitted,
  in-review, interrupted) → **defer**, never move. This is the W2-155-moves-
  itself hazard directly: the running migration task must not migrate its own
  issue mid-flight. Only settled issues move. (Matches the wave-memory
  bootstrap-boundary lesson: relaunch active work from a clean context.)
- **Idempotent / restart-safe.** Skip any issue already in the target team (its
  identifier already carries the target key). Re-running performs no duplicate
  move. No local ledger needed — the target-team check *is* the idempotency key.
- **Traceability.** On each successful move, post a Linear comment recording the
  prior identifier (`was W2-155`), then re-sync the snapshot so the cached
  `issue_identifier` (snapshot payload + session_context `LinearIssueSnapshot`)
  reflects the new value. This is the "traceable identifier change" the Proof
  requires, given renumbering makes the number itself non-traceable.

### Completed W2 history — policy: **preserve as historical**

Closed `W2-N` issues are referenced by **shipped records**: merged PR titles
(`W2-141: …`), commit messages, `MEMORY.md`. Those are immutable
(`shipped-records-immutable` feedback). Migrating closed issues would renumber
them (`W2-141` → `PRD-3`) and orphan every one of those references while buying
nothing — the work is done. **Decision: completed W2 issues stay in the shared
team as historical.** `reteam` moves only *open, settled* issues. Document this
in the migration command's help and the PM README. (The task contract explicitly
sanctions "preserve it as historical.")

### Consistency pass (spread across both PRs)

- **Swift**: the Mac reads identifiers straight from the snapshot
  (`BacklogItem`, `WaveDetailPane`), so re-sync propagates new prefixes with no
  Swift schema change. Only add a team/prefix DTO field if a surface must *show*
  the team — not required by the Proof; skip unless a view needs it (avoid
  speculative DTO fields per the DTO rule).
- **`lf pm doctor`**: extend the `pm_sync(plan:true)` diff to check team-binding
  health — every PM-enabled wave has a resolvable `pm.linear_team`, no two waves
  share a team, and flag issues stranded in the old team under a migrated wave's
  Projects (candidates for `reteam`).
- **Docs**: PM README gains the team-per-wave model, `pm init --team-key`, the
  `reteam` migration + dry-run, and the completed-history policy.

## PR decomposition (one worktree, serial)

1. **`pm-per-wave-team`** — `pm.linear_team` binding + `pm init --team-key`
   (create/adopt/diagnose) + `build_client` reads the wave team + doctor
   binding-health check + README. Delivers the forward-cut and the demoable
   PRD-/INF-/INT- win. Existing issues untouched and still readable.
2. **`pm-reteam-migration`** — `move_item_to_team` op + `lf pm reteam` (dry-run
   default, protect active Sessions, idempotent, traceability comment + snapshot
   refresh) + completed-history policy doc + doctor stranded-issue check.

Splitting this way lets PR 1 ship and demo the core user story independently of
the riskier migration; PR 2 is the state-move that must defer the very Task
running it.

## Verification target (for pursue)

- **PR 1**: unit — GOAL.md round-trips `pm.linear_team`; `build_client` picks the
  wave team over global config; `pm init` conflict/adopt/create branches (mock
  Linear via the existing `pm::test_server`). Live proof — init product/infra/
  intel with distinct keys, create one real task in each, assert the identifier
  prefixes.
- **PR 2**: unit — `reteam` dry-run lists moves + deferrals; a non-terminal
  Session defers; a second run is a no-op (already-in-target skip). Live proof —
  dry-run names issues + protected deferrals; a real move renumbers, comments
  the old identifier, and Session lookup by UUID still resolves; `pm sync` +
  `pm doctor` finish clean.

## Open question (proceeding with a decision)

Recorded in `scratch/questions.md`: renumbering means the migration is a
forward-cut + traceability, not a `W2-N → PRD-N` rename. Proceeding on
preserve-completed-as-historical + move-only-settled-open. If Jack wants number
preservation, that's a different mechanism (delete+recreate with pinned numbers,
which loses UUIDs and Session ownership — rejected here as strictly worse).
