# W2-138 — Guarantee every Task PR contains only that Task's work

## User-visible outcome

When a worker (or a human) runs `lf pr submit` / `lf pr land` on a Task
worktree, the resulting PR range is exactly the Task's own commits — never an
inherited canonical-main commit, a sibling Task's change, or hidden stale
ancestry. If the branch's range cannot be proven clean, the command **stops
before any GitHub side effect**, names the foreign commits/files, and prints the
safe rebase/recovery action. A human can trust GitHub's range, `lf task
changes`, and the Task's recorded base to agree without reconstructing git
history.

Two contamination shapes are retired for good: W2-132/#877 and W2-130/#882 both
inherited a foreign canonical-main commit because their worktree was cut from a
**local** `main` that carried an unpushed commit.

## Source of truth

`TaskPr.base_commit` (persisted in the control-plane sqlite store, per
`TaskPr`) plus git commit ancestry. `sequence` + `branch` on the same row carry
the serial-PR order. These are the durable placement/continuation evidence this
Task **consumes** — we do not invent a second stack or PR-state model (honoring
the W2-93 stacked-ancestry / W2-172 serial-continuation boundary). Readable
branch names stay hints only.

Derived views bound by the invariant: `lf task changes` (`base_commit..HEAD`,
`ops/task.rs::changes_snapshot`), the GitHub PR range, and the Mac's PR view.

## The invariant (git-native, one equality)

Let `B` = recorded `TaskPr.base_commit`, `O` = current `origin/<default>` tip,
`H` = branch HEAD, and `M = git merge-base O H`.

**Parity invariant: `M == B`.** When it holds, GitHub's range (`M..H`) equals the
recorded range (`B..H`) equals `lf task changes` — proven, not asserted.

The four ways it can fail map exactly onto the five required test scenarios:

| Relation | Meaning | Verdict |
|---|---|---|
| `M == B` | fork point is the recorded base | **SAFE** — parity holds |
| `M` is a strict ancestor of `B` (`M < B`) | branch forked *before* the recorded base; commits `M..B` are in the GitHub range but absent from `lf task changes` | **CONTAMINATED** — the #877/#882 shape; refuse |
| `B` is a strict ancestor of `M` (`B < M`) | origin advanced past a stale/superseded base (origin moved forward, or a parent PR squash-merged so `B`'s content is already on `O`) | **SAFE but stale** — proceed and heal `base_commit → M` |
| neither reachable (`B`, `M` diverge) | genuinely ambiguous ancestry | **AMBIGUOUS** — refuse |

`M < B` is precisely "the recorded base itself carries commits not on origin" —
inherited foreign ancestry. `git rev-list M..B` names those foreign commits;
`git diff --name-only M..B` names the foreign files. Safe action printed:
`git rebase --onto origin/<default> <B> <branch>` (or re-cut from
`origin/<default>`).

The `B < M` branch is what makes the "stale recorded base" and "squash-merged
parent" scenarios open the intended **minimal** range: `M..H` drops the
already-merged commits, and healing the recorded base to `M` keeps `lf task
changes` truthful.

## What already exists (do not rebuild)

- **Placement** (`ops/task.rs::task_run`, ~337-344) already fetches
  `origin/<default>`, sets `base_ref = origin/<default>`, records
  `base_commit = rev-parse(origin/<default>)`, and creates the worktree from it.
- **Rotation** (`ops/task.rs`, ~1636-1642) already re-fetches origin and cuts
  PR N+1 from `origin/<default>`, recording a fresh `base_commit`. Multi-PR
  continuation is satisfied by construction.
- `lf task changes` already reports `base_commit..HEAD`.
- Publication (`ops/land.rs::prepare_pr`) already rejects the control-plane PR
  and rebases onto `origin/<default>`.

## What this Task builds (three gaps)

### Gap A — Pre-publication range proof (the core deliverable)

New Task op `verify_task_pr_range(repo)` in `ops/task.rs`, called from
`ops/land.rs::prepare_pr` **at the very top, before `prepare_land`** — because
`prepare_land`'s `commit_workflow` runs with `push: true` /
`create_draft_pr: true`, so it is the first GitHub side effect and refusal must
precede it.

Behavior:
1. Resolve the Task session for the worktree. **No session ⇒ `Ok(())`** (plain
   non-Task PRs are unaffected).
2. Fetch `origin/<default>` fresh; read `B` from the active `TaskPr`, `M`,
   `H`.
3. Evaluate the parity table:
   - `M == B` → ok.
   - `M < B` → `Err` naming `rev-list M..B` commits + `diff --name-only M..B`
     files + the `git rebase --onto` action. **Refuse before any push.**
   - `B < M` → ok; heal `base_commit = M` on the `TaskPr` (lease-guarded
     write, mirroring `reconcile_task_pr_with_authority`), so the durable
     evidence and `lf task changes` stay truthful.
   - divergent → `Err` ("ambiguous ancestry") + recovery action.
4. Assert the proposed range `M..H` is non-empty (branch contains Task-authored
   changes) — else the existing "no changes to publish; complete directly"
   error.
5. No-remote repository: `git remote` empty ⇒ compare against local
   `refs/heads/<default>` instead of `origin/<default>`; the same table applies
   with `O` = local default tip.

### Gap B — Placement control-plane guard + no-remote fallback

In `task_run` placement (and the rotation path):
- **No-remote fallback:** guard the `fetch(origin, …)` call. If no remote
  exists (`git remote` empty), skip the fetch and record
  `base_commit = rev-parse(refs/heads/<default>)` with `base_ref` = local
  default. Explicit, not an error.
- **Ahead-of-upstream stop:** after fetch, if `origin/<default>..<default>`
  (local default has commits not on origin) is non-empty, **refuse placement**:
  "canonical <default> is ahead of origin/<default> by N commit(s) [names];
  this is a control-plane violation — push or reset canonical main before
  placing Task worktrees." This stops contamination at the source (the #877/#882
  root cause) rather than only catching it at publication. Lives alongside
  `ensure_clean_main`, which today checks branch+cleanliness but not ahead-ness.

### Gap C — Wire the guard into `request_task_pr_publication`

`request_task_pr_publication` stays the durable publication-intent writer; the
new `verify_task_pr_range` is the gate that runs earlier. Keep them separate:
one proves the range, the other records the intent. (No new DTO fields — the
`ci_observation`/`publication` shape is untouched.)

## End-to-end proof

**Scenario (contamination, the acceptance case):** Build a bare origin at commit
`P`. Local `main` gains an unpushed commit `X` (ahead of origin). A Task worktree
is placed — with Gap B, placement **refuses** ("canonical main ahead of origin");
with the guard bypassed for the test, the branch carries `X`, and `lf pr submit`
**refuses before push** with `M(=P) < B`, naming `X` and the rebase action.
After `git rebase --onto origin/main <X> <branch>`, re-run: `M == B'`, submit
proceeds, and GitHub's range == `lf task changes` == `{task commit}` only.

Command/observation that proves it: an integration test asserting (a) placement
refuses ahead-of-origin, (b) submit refuses with the foreign commit named and
**no `git push` / `gh pr` issued**, (c) after the prescribed rebase, the range is
minimal and the three views agree.

## Absent / error states

- No Task session for the worktree → verification is a no-op (non-Task PRs).
- No remote → local-default fallback (both placement and verification).
- Empty range (`M..H` empty) → "no changes to publish."
- Foreign / ambiguous ancestry → refuse before push, named commits + action.
- Stale-but-compatible base (`B < M`) → proceed + heal recorded base.

## Operational boundary

Verification is git-only (`merge-base`, `rev-list`, `diff --name-only`) plus one
`git fetch` — no network beyond the fetch the publication path already performs.
Runs before the first push; adds no GitHub round-trip.

## Proof tests (integration, `rust/loopflow/tests/`)

Reuse the bare-origin + clone fixture pattern already in `worktree_tests.rs`
/`pr_tests.rs`. Five cases, each asserting range + whether a push happened:
1. ahead-of-origin canonical main → placement stops; branch-with-`X` submit
   refuses pre-push.
2. stale recorded base (origin advanced) → safe; base healed to `M`; minimal
   range.
3. squash-merged parent → safe; already-merged commits dropped; minimal range.
4. no-remote repository → local-default base; submit proceeds.
5. multi-PR continuation → PR2 cut from origin; `M == B`; parity.

Plus the ten-dogfood-PR field proof from the contract: over real Task PRs,
GitHub's range, `lf task changes`, and `base_commit` agree with zero manual
commit dropping.

## Status

- **PR1 (this branch) — Gap A shipped.** `verify_task_pr_range` +
  `verify_task_pr_range_with_authority` (`ops/task.rs`) enforce the `M == B`
  parity proof, wired into `ops/land.rs::prepare_pr` before the first push.
  Contamination (`M < B`) and divergence refuse before any GitHub side effect,
  naming foreign commits/files + the `git rebase --onto` recovery. Stale/
  squash-merged bases (`B < M`) heal forward via a new dedicated
  `heal_task_pr_base` store write (base_commit is optimistic-identity in the
  generic update, so healing needs its own keyed write). No-remote falls back to
  local `<default>`. Six unit tests cover all five Proof scenarios (parity,
  contamination-refuse, stale-heal, squash-heal, no-remote, rotated
  continuation).
- **PR2 (next serial) — Gap B, placement guards.** In `task_run` placement +
  the rotation path: explicit no-remote fallback (skip fetch, record local
  `<default>`), and the ahead-of-upstream control-plane stop before placement.
  This stops contamination at the source; PR1 is the durable publication
  backstop.

## Exclusions

- No new stack/PR-state model — consume `TaskPr {base_commit, sequence,
  branch}` (W2-93 / W2-172 own those models).
- No change to `ci_observation` / CI-fix wake (W2-156).
- No Mac/iOS UI work; the shared invariant makes their existing PR views correct
  without surface changes.
- Not rewriting `rebase_land` — it stays the rebase mechanism; we only gate it.
