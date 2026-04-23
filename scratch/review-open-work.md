# review-open-work

Add built-in tooling to get to "inbox zero" on outstanding work: a step that surveys all in-flight branches/PRs/waves and discusses triage with the user, a headless step that takes a single branch from wherever-it-is to merged, and an `lf op` for mass-pruning remote branches.

## What exists after this

```bash
lf review-open-work       # headless scan → interactive discussion
lf ship                   # headless: take branch to merge-ready and land
lf op branches list       # preview remote branches by filter
lf op branches prune      # delete remote branches by filter
```

## Shape

**Four artifacts:**

| Artifact | Type | Where |
|---|---|---|
| `review-open-work` | interactive step | `rust/loopflow/src/engine/builtins/steps/interactive/review-open-work.md` |
| `refresh-plan` | headless step | `rust/loopflow/src/engine/builtins/steps/plan/refresh-plan.md` |
| `ship` | flow | `rust/loopflow/src/engine/builtins/flows/code/ship.yaml` |
| `lf op branches` | op subcommand | Rust op crate (wherever the existing `lf op` lives) |

**Composition over duplication.** `ship` is not a new step — it's a flow that composes existing building blocks. The only net-new step is `refresh-plan`, which updates `scratch/<branch>.md` to reflect post-rebase reality. Downstream steps (`implement`, `gate`, `op: land`) already exist and already read scratch/. The scratch-doc contract is how these compose.

---

## `review-open-work` (interactive step)

### Intent

> "first try to get critical things into main, and eliminate any cruft. Then from the cleanest possible main, do a wave by wave review of what has a lot of outstanding work planned, least demonstrated success, etc"

Two passes with an interactive discussion wrapped around them. Pass 1 clears the decks (branches/PRs/worktrees). Pass 2 audits waves against the now-cleaner state.

### Frontmatter

```yaml
---
interactive: true
requires: none
produces: scratch/open-work.md, possibly dispatched ship runs and branch prunes
---
```

### Workflow

1. **Scan** — headless at the top of the step. Gather:
   - Local worktrees: `lf op wt list` (include last-commit date, branch, whether PR exists)
   - Open PRs authored by the user: `gh pr list --author @me --state open --json ...`
   - Remote branches authored by the user: `git for-each-ref refs/remotes/origin/ --format '%(refname:short) %(committerdate:short) %(authorname)'` filtered to user
   - `wave/` entries: each wave's README + roadmap priority counts + associated open PRs
   - Merge status for each branch (ahead/behind main, CI status if PR exists)
   - **Wave attribution for each worktree/branch**: use the canonical engine resolver. Worktree → `wave_name_from_worktree_and_main()`. Branch-only (no worktree) → `wave_name_from_branch()` with the configured schema. `None` from both = waveless. Don't re-derive.

2. **Write initial triage** to `scratch/open-work.md`:

   ```markdown
   # Open Work — <date>

   ## Pass 1: Clear the decks

   | Item | Kind | Age | Status | Recommendation | Why |
   |---|---|---|---|---|---|
   | jack.foo.20260401 | worktree+PR | 21d | CI green, 1 nit | **ship** | Reviewed, only polish left |
   | jack.bar.20260318 | branch only | 35d | diverged 40 commits | **abandon** | Superseded by wave/foo #3 |
   | origin/stale-experiment | remote only | 90d | — | **prune** | No local worktree, no PR |

   ## Pass 2: Wave audit

   | Wave | Vision progress | Recent activity | Recommendation |
   |---|---|---|---|
   | redesign | 2 of 4 chord pillars shipped, third in flight | 3 PRs in 2wk, all tied to roadmap | **continue** |
   | chatgui | Core UX still not usable; goals unmet | 14 commits, mostly refactors, no feature delivery | **busy without progress — reduce scope or abandon** |
   | pm | OAuth migration complete, original goal met | 1 small PR last 4wk | **wind down, goal reached** |
   ```

   **The audit evaluates progress toward wave goals, not activity counts.** Read each wave's README Goals/Vision. Judge whether recent work actually advances the wave or churns. "Busy without progress" is a distinct and important finding — call it out explicitly. Also call out **lack of action** on waves that should be moving. Commit-counts and PR-counts are signals, not the answer.

3. **Discuss** — interactive. Walk Pass 1 row by row:
   - Confirm recommendation, or override
   - If **ship**: agent shells out `lf ship` in the target worktree as a **background** process (no blocking, no waiting). Moves to the next row immediately. Agent tracks which ships were kicked off so it can report at session end.
   - If **abandon**: agent deletes the branch locally + remote after user confirms
   - If **prune**: batch these for a single `lf op branches prune` at the end

   **Dispatch mechanics.** The agent uses its shell tool's background-execution capability (or `nohup ... &` + redirected log file — whichever the host agent supports). No new loopflow infra. No formal sub-step construct. Just the agent orchestrating parallel `lf ship` processes across sibling worktrees. Log files go to `scratch/ship-logs/<branch>.log` so the user can tail.

   **Recommendation vocabulary differs by whether the branch has a wave:**
   - Waveless branch: **ship** or **abandon** only. No "defer" — waveless branches don't spawn new waves.
   - Waved branch: **ship** (finish cleanly), **abandon** (drop the branch, wave's roadmap keeps the intent), or **ship-partial** (ship what's done, defer leftovers into the wave's roadmap — this is the ship step's headless default when dispatched).

4. **After Pass 1 actions complete** (or user says "move on"), walk Pass 2:
   - For each wave: continue / split / reduce scope / archive
   - Record decisions inline in the scratch doc

5. **End** — update `scratch/open-work.md` with final decisions (struck-through rows for done items, remaining items as a punch list). The scratch doc becomes the checkpoint if the session crashes.

### What makes this step different

It **dispatches** other commands. Most interactive steps gather / write / discuss. This one actually spawns background `lf ship` processes in sibling worktrees and runs `lf op branches prune` as the session progresses. The step prompt must be explicit that:

- Ship dispatches are fire-and-forget: kick off, log to `scratch/ship-logs/<branch>.log`, move on. Don't wait.
- Multiple ships can run in parallel across different worktrees.
- At session end, the agent reports what was dispatched and where the logs are. It doesn't tail or summarize ship outcomes — those are separate runs the user reads asynchronously.

No new loopflow infra needed. The agent uses shell job control.

### Inferring archival conventions

Repos develop conventions for parking old waves — `wave/old/`, name prefixes, empty roadmap files, a stale README. The step shouldn't hardcode any of these. Instead:

- Scan `wave/` and read each entry's structure
- Flag waves that look archived (nested under a parent like `old/`, no roadmap items, README explicitly says deprecated, no recent commits touching the area)
- Treat these as low-priority in the audit and note the inferred reason
- If ambiguous, ask the user in the discussion phase: "Looks like `wave/old/*` is your archive convention. Skip these?"

The heuristic is human-readable and can be wrong — the discussion phase is where the user corrects it.

### Not in scope

- Automating wave mutations (split/archive). Recommend; don't do.
- Scanning teammate branches. User's own work only — `--author @me`.
- Evaluating unlanded PRs from others. That's review work, not inbox-zero.

---

## `refresh-plan` (headless step)

### Intent

Before evaluating "what's done vs what's planned," the plan needs to reflect current reality. Branches sit. Main moves. Upstream refactors can ship part of what scratch describes, or invalidate the rest. Refresh reconciles.

### Frontmatter

```yaml
---
requires: current branch is not main
produces: scratch/<branch>.md (created or mutated)
---
```

### Workflow

1. Rebase onto main (`lf op rebase`). Trivial conflicts: resolve. Non-trivial: bail to `scratch/questions.md`.
2. Read or synthesize the scratch doc:
   - Existing doc: reconcile against post-rebase diff and upstream changes touching the same area. Strike through items already shipped elsewhere, note upstream changes that invalidate assumptions, restate what's still outstanding.
   - No existing doc: synthesize one from the diff + branch name + PR body. Describe what this branch *is* doing, inferred from evidence.
3. Output: a fresh `scratch/<branch>.md` that accurately describes "planned vs done, post-rebase."

### Why it's its own step

Because refresh writes scratch, downstream steps that read scratch (`implement`, `gate`, etc.) compose naturally. Anyone can chain `refresh-plan → <their own flow>`. The scratch-doc contract is the integration surface — ship isn't the only flow that benefits.

### Standalone uses

- After a long rebase on a wave branch, before resuming work
- Before handing a branch off to a teammate
- Before a design-review session on in-flight work

---

## `ship` (flow)

### Intent

> "non-interactive flow where the design/review stages are replaced with instructions to evaluate what's already implemented, what was planned on the branch, and what is blocking being merged in. Implement anything planned and straightforward but default towards scoping work into later wave docs and prioritize getting merged quickly."

Decisive "get this branch to main." Composes existing steps.

### Definition

```yaml
# flows/code/ship.yaml
flow:
  - refresh-plan
  - implement
  - gate
  - op: pr          # create or update PR
  - op: land
```

### How the bias-toward-landing lands

The user's framing — "implement anything planned and straightforward but default towards scoping work into later wave docs" — lives in the `refresh-plan` prompt. When refresh-plan is invoked as part of the `ship` flow, its output scratch doc ends with an explicit "Strategy" block that `implement` reads and honors:

```markdown
## Strategy: ship bias

- Finish only what's trivial and in-scope for this branch
- Defer anything non-trivial into the wave's roadmap (or `scratch/questions.md` if waveless)
- Prefer landing over comprehensive — a wave doc captures intent
```

The refresh-plan prompt detects whether it's running inside the `ship` flow (e.g., via an env var or flow context) and conditionally appends the Strategy block. When refresh-plan is used standalone, no Strategy block — it's just a plan refresh.

No new direction. The bias lives in the prompt.

### Wave deferral

Handled by implement via the scratch contract + the engine-injected `<lf:wave>` tag (resolver: `wave_name_from_worktree_and_main()` at `rust/loopflow/src/engine/worktrees.rs:82`, fallback `wave_name_from_branch()` at `naming.rs:121`, injected by `prompt.rs:1668`). `refresh-plan` writes "defer X into wave" into scratch; implement reads that and appends to `wave/<name>/` when waved, or writes `scratch/questions.md` when waveless and leftover is non-trivial.

### Guardrails

- **No new waves.** Waveless branches ship as-is or abandon. If implement wants to create `wave/<slug>/`, stop and write `scratch/questions.md` instead.
- **If nothing is mergeable** (broken, mis-scoped): gate fails, flow halts, scratch/questions.md holds the explanation. Do not destroy work to force a ship.

### Output

- Branch landed to main, or
- `scratch/questions.md` explaining why not. Deferred work lives in `wave/<name>/N-*.md`, committed as part of the shipping branch.

---

## `lf op branches` (op command)

### Commands

```bash
lf op branches list [filters]      # preview what would be deleted
lf op branches prune [filters]     # actually delete, with confirmation
```

### Filters

| Flag | Example | Meaning |
|---|---|---|
| `--user <name>` | `--user jack` | Branches authored by user |
| `--user @me` | | Current git user (default if no filter at all?) |
| `--wave <name>` | `--wave redesign` | Branches whose name includes `<wave>` segment |
| `--stale <duration>` | `--stale 30d` | No commits in last N days |
| `--created-before <date>` | `--created-before 2026-01-01` | Branch first-commit date before cutoff |
| `--merged` | | Branches merged into main |
| `--dry-run` | | Implied by `list`; explicit on `prune` for safety |

Combining filters = AND. Default `prune` requires at least one filter (no foot-guns).

### Safety

- Never delete `main`, `master`, current branch, or any branch with an open PR (unless `--include-open-prs` is passed)
- `prune` prompts with the full list unless `-y` / `--yes`
- Deletes are remote-only (`git push origin --delete <branch>`). Local branches untouched — loopflow owns worktrees, not branches.

### Why an op, not a step

Pure mechanical operation, no LLM judgment needed. `lf op` already hosts this category (`wt`, `pm`, `auth`, `land`, etc.). Fits there.

---

## Naming decisions

- Review step: `review-open-work` — matches user's language, becomes `lf review-open-work`
- New plan step: `refresh-plan` — describes what it does to scratch
- Ship: `ship` — flow, not step. `lf ship` invokes the flow.
- Op: `lf op branches` — parallel to `lf op wt`

Alternatives considered:
- `lf inbox-zero` for the review step — cute but jargony
- `reconcile` / `replan` for refresh-plan — reconcile is accurate but jargony; replan overclaims (this isn't starting over)
- `lf finalize` / `lf close-out` for ship — less clear than `ship`
- `lf op remote-branches` — redundant, branches are always remote in this op

---

## Resolved design decisions

- **Dispatch**: agent shells `lf ship` in target worktree as a background process. No new infra. Logs at `scratch/ship-logs/<branch>.log`.
- **Waveless branches**: ship or abandon. No new waves spawned by `ship`. Non-trivial leftover → `scratch/questions.md`.
- **Ship bias**: lives in the `refresh-plan` prompt (conditional on ship-flow context). No new `ship-bias` direction.
- **Wave audit criterion**: progress toward wave Goals/Vision, not activity counts. Call out "busy without progress" and "lack of action" explicitly.
- **Wave attribution**: canonical engine resolver. Step doesn't re-derive.
- **Stale scratch docs**: out of scope for v1.
- **Commit split**: one branch, one PR.

---

## Done when

- `lf review-open-work` runs on this repo, produces a coherent `scratch/open-work.md`, and walks through the items interactively
- `lf ship` on a branch with a small leftover lands it (or writes `scratch/questions.md` if genuinely stuck)
- `lf op branches list --user @me --stale 60d` previews candidates; `lf op branches prune` with same filters actually deletes after confirmation
- README updated under the "Steps" and "Ops" tables
