# LOO-552: Default Git planning to one shared repository ref

Status: implemented 2026-10-21; gate, demo and Jack's review of the assumed
choices remain. Jack Heart requested the idea on 2026-10-21: "lets try the idea of
having a default global ref." The open choices below are assumed in
`scratch/questions.md` and still need Jack's review — decision 3 now conflicts
with an earlier accepted constraint (see below).

## Problem

With no `planning:` in `.lf/config.yaml`, Git planning publishes to
`refs/loopflow/planning/users/<user-key>` (`PlanningConfig::Git { shared: None }`,
`rust/loopflow/src/config.rs:214-249`; ref chosen in
`rust/loopflow/src/ops/planning_sync.rs:86-123`). A second person who clones the
repository sees none of the first person's Tasks, and theirs stay invisible.
etude is on Jack's personal ref today (`origin` has
`refs/loopflow/planning/users/9a0c1265-…`, no shared ref). LOO-550's docs review
found this the top problem for two people in one repository.

Beneficiary: anyone who brings a second person into a repository that plans on Git.

## Demo

Two Homes, one bare remote, no `planning:` config:

```bash
# Home A (Maya)                          # Home B (Lee), fresh clone
lf task create "GPU fallback" …          lf task status lf-3f2a1c --json   # same id, title, comments, owner
lf task claim lf-3f2a1c                  lf comment lf-3f2a1c "keep CPU path"
lf comment lf-3f2a1c "start here"        # → appears in A's `lf task status`
lf planning status   # refs/loopflow/planning/shared/main, active
```

Migration demo on a disposable etude: `lf clone create` of Jack's Home, a
`git clone --mirror` of etude's origin (it carries the personal ref), a checkout
of that mirror relocated with `lf repo identity --relocate <repository-id>`.
Running the new `lf task list` there publishes every etude Task to
`shared/main`; a fresh Home cloned from the mirror then lists the same Tasks,
identifiers, comments and owners. Jack's real Home and etude remote are untouched.

## Risk findings

- **Shared refs already support concurrent writers.** Publication always fetches
  first, merges the remote snapshot with the local export by mutation identity
  and causality, and pushes a commit parented on both (`publish_destination`,
  `planning_sync.rs:251`; `concurrent_publication_preserves_both_revisions_…` in
  `tests/planning_git_tests.rs`). Two-Home exchange over one shared ref is already
  proven (`public_work_connections_exchange_offline_edits_…` in
  `tests/planning_foreground_tests.rs`). The default change adds no transport work.
- **Migration is already automatic.** `export_peer_planning` with
  `include_current=true` exports every mutation of every Wave/Project/Task/comment
  this Home owns for the repository, regardless of which destination it came
  from (`OWNED_CHANGES`, `store/sqlite/planning_peers.rs:445-498`; proven by
  `routing_retains_each_destination_across_switches_moves_and_missing_work`).
  Mutation ids, clocks and parents travel unchanged, so identity and history
  survive. The old personal destination stays bound, inactive and readable in
  `lf planning status`. Execution (worktree, PR, Workflow, Sessions) never
  travels, so started work is untouched. **No migration command or shim.**
- **Task deletion survives the switch.** Deletion is a field
  (`WorkValue::TaskDeletedAt`), so a deleted Task stays owned and its tombstone
  exports. Waves are hard-deleted rows; a second machine of the same user that
  never fetched a Wave deletion could republish that Wave. Accepted: rare, and
  equally possible today when switching to `shared:`.
- **Identifiers do not collide.** Git-planned Tasks use `lf-<hex>` derived from
  the Task UUID (`store/sqlite/children.rs:89`); Waves share `GOAL.md` ids.
- **Version skew.** A machine still on an older lf keeps publishing to the
  personal ref. Its edits reach `shared/main` on its first publish after upgrade.
  Nothing is lost; until then a teammate does not see them. Docs say so.
- **Privacy is unchanged.** Ref separation was never access control (docs/lf.md
  already says so); a personal ref on `origin` was readable by anyone with fetch.
- **LOO-550 overlap.** LOO-550 (unmerged, `../loopflow.loo-550-take-a-stab`)
  rewrites "Choose planning synchronization", adds "Work with another person",
  `docs/getting-started.md` and glossary rows that describe the personal default.
  Whichever lands second rebases its docs onto the other; this Task owns the
  default's wording.

## Decisions (assumed 2026-10-21, pending Jack)

1. **Default ref:** `refs/loopflow/planning/shared/main` — the existing shared
   namespace with name `main`. Explicit `shared: <name>` keeps working.
2. **Personal is explicit:** `planning: {provider: git, personal: true}` in
   repository or `~/.lf/config.yaml`. `personal` with `shared` is a config error.
   The planning key stays, used only by personal refs; `lf planning key` is
   unchanged and still provisions lazily on first personal use.
3. **Existing personal-ref repositories migrate automatically** on the first
   publish after upgrade (finding above). A person who wants to stay personal
   commits or sets `personal: true`. The old remote ref is left in place;
   deleting it is an ordinary `git push origin :<ref>`, not an lf command.

**Open conflict with LOO-412 (found at realign, 2026-10-21).** Infrastructure
memory records Jack's LOO-412 transport decision: user-keyed Git default, shared
opt-in, and "joining cannot silently publish existing plans"; Product memory adds
"code remotes grant neither identity nor real-data publication permission."
Decision 3 does exactly that silent publication: an unconfigured Home's personal
plan is merged into `shared/main` on its next save. Jack's new request reverses the
default but does not say whether that constraint goes with it. Choices:

- (a) Accept the automatic move (current code). Shared default implies existing
  unconfigured plans join it; docs say so. Simplest; matches "no migration shim."
- (b) Shared default for new repositories only: a Home whose store already has a
  bound personal destination for the repository keeps publishing there until
  config selects one. This is the "migrated marker"/fallback the plan forbids,
  under another name.
- (c) Shared default, but existing personal history stays unexported until an
  explicit action (`personal: false`-style config or a command). Needs a new
  export-scope rule in `OWNED_CHANGES`.

Unresolved; the implementation is (a).

Concept review (2026-10-21, no person present) confirmed the model: one config
value (`shared: <name>` | `personal: true`, default `shared/main`) maps 1:1 to
`GitPlan`, and destination switching already carries identity, so nothing else
needs a new concept. The product memory line "code remotes grant neither identity
nor real-data publication permission" was already relaxed by the old user-keyed
default publishing to `origin`; it adds no separate question. The one open product
choice is the question above, exactly: **when an unconfigured Home already has a
personal plan for a repository, does upgrading publish that plan to
`shared/main` on its next save (a), keep it personal until config chooses (b), or
start shared/main empty for that Home's old history until an explicit action (c)?**
Evidence: infrastructure MEMORY LOO-412 paragraph ("joining cannot silently publish
existing plans") vs Jack's 2026-10-21 request, which names the default but not
existing plans. Implementation stays (a) until Jack answers; (b)/(c) each touch
routing or `OWNED_CHANGES`, not the config type.

Alternatives considered: a new `refs/loopflow/planning/repository` namespace
(one more naming rule, no benefit); a `lf planning migrate` command (duplicates
what switching already does); a single `ref:` field with a reserved `personal`
value (collides with a shared ref named "personal").

## Architecture

Before: `PlanningConfig::Git { remote, shared: Option<String> }`; `None` means
personal. Two hand-copied `match shared` blocks in `planning_sync.rs`
(`configured_destination`, `peer_status`) build the ref.

After: `PlanningConfig::Git { remote, plan: GitPlan }` with
`enum GitPlan { Shared(String), Personal }`, `Default` = `Shared("main")`.
YAML stays `shared:` / `personal: true`, mapped through a private raw struct
(`#[serde(try_from, into)]`) that rejects both. One `GitPlan` method builds the
reference; both sync call sites use it. Source of truth stays the repository
config plus the local SQLite planning store; status remains derived.

Implementation note: `GitPlan::reference(user_key)` returns `None` for a personal
plan without a key, so passive status never provisions one; publication provisions
through `provisioned_user_key` in `planning_sync.rs` first. AGENTS.md's stale
`scripts/generate_cli_reference.py` line now names the `UPDATE_CLI_REFERENCE` test.

Affected surfaces: `config.rs`, `ops/planning_sync.rs`,
`lf/commands/planning.rs` (status headline names the ref kind instead of
"user-keyed Git"), `PlanningCommand::Key` help ("personal ref"),
`docs/lf.md` "Choose planning synchronization", `docs/architecture/planning.md`,
regenerated `docs/lf-reference.md` if help text changes. Wire DTO
(`peer_planning_status.json`) and Desktop are unchanged: they already carry
`reference` as a string.

Absent/error states: missing config → shared/main; `personal: true` with no key
→ key provisioned as today; `shared` + `personal` → config load error naming both
keys; unreachable remote → local save, pending publication (unchanged).

## Delete — do not maintain

- `shared: Option<String>` and its "Omit for this user's stable personal ref"
  comment; `None`-means-personal in both `planning_sync.rs` match blocks.
- The duplicated ref formatting in `configured_destination` and `peer_status`.
- `docs/lf.md` text "Planning defaults to your Git ref" and "Omit `shared` for
  `refs/loopflow/planning/users/<user-key>`".

Forbidden: a migration command, a fallback that reads the personal ref when the
shared one is empty, dual publication to both refs, or a per-repository
"migrated" marker.

## Slices

One slice (this slice): config type + sync call sites + status text + tests + docs.
Focused test: `cargo test -p loopflow --test integration planning_setup`
(or the planning test binary it lives in) plus the config unit test.

Tests:
- Config unit test: default is `Shared("main")`; `personal: true` → `Personal`;
  both → error; Linear unchanged.
- `planning_setup_tests`: rename `personal_git_is_automatic_…` to
  `personal_git_is_explicit_…` (config `personal: true`); add
  `default_git_plan_is_the_shared_main_ref` (no config → `shared/main`, no key).
- New integration test `personal_plan_moves_to_shared_default_with_identity`:
  Home A with `personal: true` creates a Task, comment and claim and publishes
  to its users ref; config removed; one ordinary command; Home B (no config,
  fresh) resolves the same Task id/identifier/comment/owner from `shared/main`;
  A's worktree/Workflow rows unchanged; A's status lists the personal destination
  inactive with its retained changes.

## Done when

- Two Homes in one repository with no `planning:` config see each other's Tasks,
  Projects, comments and ownership after ordinary commands.
- A repository on a personal ref moves its Tasks to `shared/main` on the first
  publish after upgrade, keeping ids, identifiers, comment history, ownership log
  and local execution.
- `personal: true` publishes to `refs/loopflow/planning/users/<key>` as before.
- `docs/lf.md` describes the shared default, `personal: true`, and the automatic
  move; no doc says planning defaults to a personal ref.

Gate: `cargo test -p loopflow` planning suites + config unit test, `cargo clippy
--all-targets -- -D warnings`; expected all pass. Demo: the etude disposable
world above (needs Jack's Home for `lf clone create`; runs on his Machine).

## Proposed Wave memory updates (not applied; memory is read-only here)

- `.lf/wave/infrastructure/MEMORY.md` LOO-412 paragraphs ("User-keyed Git is the
  default; shared planning is opt-in…", "Jack selected Linear, shared or default
  user-keyed Git…") and `.lf/wave/product/MEMORY.md` ("Jack selected user-keyed Git
  by default, explicit shared opt-in…"): on landing, record that Jack's 2026-10-21
  LOO-552 request made `shared/main` the default with `personal: true` opt-in, and
  how the silent-publication conflict above was resolved.

## Check results

Compress (2026-10-21, reused at realign; realign changed only prose): `cargo test` lib planning_config_tests + integration planning_setup/personal_plan_moves, `cargo clippy -p loopflow --all-targets -D warnings`: pass. Gate owns full planning suites; demo on the disposable etude world (Jack's Machine).
