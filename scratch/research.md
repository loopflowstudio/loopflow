# Research: M1 Goals Architecture Landing

## System understanding

M1 is a dependency-direction cleanup around the live wave-agent architecture, not a product rewrite. The current code already behaves like the new model in several places:

- `lf wave <name>` boots a listener and resident pair under `rust/loopflow/src/wave/`.
- `lf q worker run` is the active daemonless dispatch path today, but Jack now wants M1 to remove that API and make dispatch a flag on ordinary `lf` execution.
- `lfd` is already mostly a local query/push surface, but several routes still mutate git/tmux/ops in process.
- The shared conversation vocabulary and vendor harness still live under `lfd::conversations`, even though the resident and wave listener are the real consumers.
- GOAL.md frontmatter parsing lives under `lfd::http::routes::wave_config`, but `wave`, `ops`, `resident`, and `lfd` all depend on it.

### Architecture

The main ownership violations before M1:

- `wave` imports `lf::commands::util::find_repo_root`.
- `wave::resident` imports `lf::commands::sub::stream_events`.
- `wave` imports `lfd::http::routes::wave_config`.
- `wave::resident` imports `lfd::executor::ensure_wave_worktree`.
- `wave` and `engine::wave_context` import `lfd::conversations::{turns, types}`.
- `lf q` imports `lfd::executor::{create_run_for_placement, Placement}` and tmux helpers.

These are real cycles across authority boundaries, not cosmetic module names.

### Data flow

Worker dispatch today:

1. `lf q worker run` parses `--flow`, `--task`, `--pool`, `--stack`, `--no-pr`.
2. `lf/commands/q.rs::dispatch` resolves the wave row, capacity, parent session, placement, worktree, run row, channel, and worker session row.
3. Worktree placement still calls into `lfd::executor::helpers`.
4. A tmux wrapper launches `lf <flow>: <task>` and clears `LFD_SESSION_INHERITED`.
5. The worker uses `LFD_CHANNEL` to report through `lf chat`.
6. The parent listener is best-effort notified through `/channels`.

Target dispatch shape under discussion:

```bash
# same target branch, separate worktree
lf implement "task" --dispatch

# stacked: create a worktree/branch based on an existing run or branch point
lf implement "task" --stack <run-id-or-branch?>

# independent branch: create a new worktree from HEAD's base
lf implement "task" --fork
```

Placement changes the worktree/cwd for the agent launched by this `lf` command. It does not detach by default: `lf` blocks on the agent as if it were running normally.

Resident flow today:

1. `lf wave <name>` starts the listener from origin repo.
2. The listener spawns `lf wave <name> --mind-only`.
3. The resident ensures and enters `<repo>.<wave>` using `lfd::executor::ensure_wave_worktree`.
4. It reads `mind:` from GOAL.md through `lfd::http::routes::wave_config`.
5. It creates a vendor harness from `lfd::conversations::harness`.
6. It subscribes to listener inbox using `lf::commands::sub::stream_events`.
7. It maps `ConversationEvent`s into resident wire deltas.

Concerto flow for hand routes:

- Swift still calls `/waves/{id}/land`, `/next`, `/combine`, `/stop`.
- The Swift service expects small JSON response DTOs and uses long HTTP timeouts for git/GitHub operations.
- The route handlers currently call `crate::ops::*`, branch/worktree rename helpers, and tmux cancellation directly.

## High-leverage questions

### 1. What replaces `lf q worker run`?

Jack's current direction: M1 is the right time to remove the `lf q worker` API and express work placement as flags on ordinary `lf` invocations.

Resolved direction:

- `lf <flow-or-step> ... --dispatch|--stack|--fork` resolves the placement, changes the agent cwd/worktree, runs the normal prompt launch there, and blocks until that agent completes.
- M1 should not preserve detached tmux dispatch as the default replacement for `lf q worker run`.
- If detached execution is still needed later, add an explicit `--detach` or a separate command after the normal placement model is clean.

This also decides whether `worker` remains an explicit CLI noun. It should not. It can remain an internal `SessionUse` if useful for existing rows or UI grouping.

Implementation consequence: prompt launch needs to resolve placement before context assembly/agent config, then use the placed worktree as `repo_root` and cwd. Session/run registration should describe that placed worktree, not the coordinator's original cwd.

### 2. What should branch placement flags mean?

Candidate grammar after Jack's latest pass:

```bash
lf implement "task" --dispatch
lf implement "task" --stack <run-id-or-branch>
lf implement "task" --fork
```

Semantic choices:

- `--dispatch` means separate worktree, same remote target branch. Technical constraint: git cannot check out the same local branch in two worktrees, so implementation likely needs a run-local branch that tracks/pushes to the same remote branch, or a detached worktree that pushes `HEAD:<target>`.
- `--stack <run-id-or-branch>` means create a stacked branch/worktree on top of an existing run or branch.
- `--fork` means create an independent branch/worktree from HEAD's base: normally `origin/<default>`, except when the current branch is stacked, where the base is the branch it is stacked onto.

Resolved name: `--fork`.

Resolved base rule for `--fork`: use the branch's review base, not `merge-base(HEAD, default)` as an abstract commit. In practice:

- Unstacked branch: fork from `origin/<default>` (`origin/main` here unless repo config says otherwise).
- Stacked branch: fork from the parent branch this branch is stacked onto.
- Main/default checkout: fork from `origin/<default>`.

Implementation should prefer portable git/PR facts over loopflow-only metadata:

1. If the current branch has an open PR, use the PR base branch.
2. Else, if local git can identify an upstream/base branch for the stack, use that.
3. Else, use `origin/<default>`.

lfdb lineage can speed this up or annotate the UI, but the branch should remain understandable outside loopflow.

Stack encoding should stay layered, with git/PR truth first:

- **Git ancestry:** `--stack X` creates the new branch from X's tip, preferring the local parent branch when it has unpushed commits and otherwise `origin/<parent>`. This is what makes the code actually build on the parent.
- **PR base:** when the stacked branch gets a PR, its base should be the parent branch, not default/main. That is the GitHub-visible stack.
- **lfdb lineage:** if X resolves to a run, store `parent_run_id`, `parent_pr_number`, `stack_group_id`, `stack_position`, `stack_status`, and `target_branch` on the child run. This is annotation/cache for queue reconciliation and Concerto, not the source of truth.
- **Inferred lineage:** if X is a branch/ref rather than a run, create the branch from that ref and set `lineage_inferred = true`; either attach to a matching run if one exists or derive the stack from git/PR ancestry.
- **Branch names:** branch names can stay human-readable/unique. They should not be the source of truth for stack structure.

### 3. What is the no-flag placement default?

The explicit placement flags are resolved:

- `--dispatch`: separate worktree, same remote target branch.
- `--stack X`: separate worktree, new branch stacked on X.
- `--fork`: separate worktree, new independent branch from the current branch's review base.

Resolved default: current cwd. Bare `lf implement "task"` runs exactly where the shell is. Placement changes only when the user passes an explicit flag: `--dispatch`, `--stack`, or `--fork`.

### 4. Should lfd hand routes become exec gateways while preserving DTOs?

Routes `/land`, `/next`, `/combine`, `/stop`, and rename are the clearest "lfd still has hands" violations. But Swift/Concerto calls them directly and expects current DTOs:

- `LandWaveResponse { merged }`
- `NextWaveResponse { new_branch }`
- `CombineResponse { ok, result }`
- `StopWaveResponse { stopped }`

Blindly replacing internals with `Command::new("lf")` loses structured results unless commands print machine-readable output or the route re-reads state after exec.

Decision needed: preserve the HTTP API and add machine-readable `lf op` output/re-read behavior, or allow a UI/API break in this M1 PR.

### 5. Is `step -> skill` part of M1 implementation or just grammar direction?

This is large. A full rename touches:

- Rust engine type names: `Step`, `FlowItem::Step`, `ConcreteItem::Step`, `load_step`.
- Prompt tags: `<lf:step:...>`.
- Paths: `.lf/steps`, builtin `engine/builtins/*/step/`.
- Flow expansion and execution.
- Python `FlowStep(type="step")`.
- Swift `Step`, `StepRun`, catalog models, wave config `stepAgents`.
- Golden prompt fixtures and docs.

Decision needed: M1 should either do a narrow user-facing grammar change and leave the internal noun for a later migration, or explicitly absorb this as a large breaking rename.

### 6. Where should turn vocabulary live?

The harness emits `ConversationEvent`/`ConversationItem`. The listener streams `ChatTurn`. Today all three live under `lfd::conversations`, but consumers are broader:

- `wave::mind`, `wave::runtime`, `wave::journal`, `wave::server`, `wave::channel`
- `engine::wave_context`
- `lf sub`
- harness drivers and conformance tests

Moving only `harness` is easy but leaves `wave` depending on `lfd::conversations::turns/types`. Moving `types` and `turns` with harness makes the harness module own listener wire vocabulary. Moving turns under `wave` makes harness depend on wave-adjacent types.

Decision needed: choose the conceptual owner for `ConversationItem`, `ConversationEvent`, and `ChatTurn` before code starts.

### 7. How much old goal-agent/render path gets deleted?

The old route-driven worker/goal-agent machinery described in follow-ups is mostly gone, but goal rendering is not dead:

- `wave::mind::build_goal_seed` uses `load_goal` + `render_goal`.
- `engine::prompt` still has wave-agent inline-run handling around `render_goal`.
- `engine::flow::render_goal` is also the resident seed renderer.

Decision needed: preserve `render_goal` as the resident seed path while deleting only inline-run leftovers, or keep all render paths until after M1 to avoid behavioral risk.

### 8. Does M1 update architecture docs after landing?

`wave/goals/architecture-direction.md` is the durable target snapshot. Updating it in the same PR makes review clearer but risks turning a design anchor into a status log.

Decision needed: mark resolved M1 bullets in `wave/goals/architecture-direction.md`, or keep status in PR notes only.

## Tensions

- **Local command truth vs HTTP UI continuity:** lfd should exec `lf`, but Concerto currently speaks structured HTTP.
- **Ordinary lf invocation vs historical worker rows:** removing `lf q worker` makes the CLI cleaner; implementation should avoid reintroducing "dispatcher" vs "worker" as a second hidden control path.
- **Clean architecture vs single-PR blast radius:** moving modules is cheap; renaming `step` to `skill` across Rust/Python/Swift/docs/goldens is not.
- **Harness ownership vs wire vocabulary ownership:** vendor events and streamed chat turns are adjacent but not the same domain.
- **Current cwd default vs branch isolation:** bare `lf` is unsurprising for humans; `--dispatch`/`--stack`/`--fork` make branch relocation explicit.
- **Deleting legacy vs preserving demo:** render_goal and route DTOs each have existing behavioral footprint.

## Recommendations

### Treat M1 as ownership + command-boundary cleanup, not vocabulary migration

**Observation:** `step -> skill` reaches every layer and many fixture/golden surfaces.

**Cost:** High; likely dominates the PR and obscures the architecture move.

**Benefit:** User-facing language becomes cleaner.

**Verdict:** Keep internal `Step`/`.lf/steps` in M1 unless Jack explicitly wants the larger breaking rename now. Add a narrow grammar/UX bridge only if it directly supports the M1 demo.

### Preserve lfd route DTOs while changing authority

**Observation:** Swift calls hand routes directly; the route response bodies are tiny but relied on.

**Cost:** Medium. Routes can exec `lf op ...`, then parse command output or re-read state to shape current DTOs.

**Benefit:** lfd stops having hands without forcing a simultaneous Concerto migration.

**Verdict:** Best M1 boundary unless the product goal is to break the API now.

### Move conversation vocabulary to a neutral module before moving harness

**Observation:** `ConversationItem`, `ConversationEvent`, and `ChatTurn` are consumed by engine, lf, wave, and harness.

**Cost:** Medium mechanical import churn.

**Benefit:** Avoids re-homing `harness` while keeping hidden `lfd` dependencies through `types/turns`.

**Verdict:** Create `crate::conversation` or `crate::chat` for shared event/turn vocabulary; move vendor drivers to `crate::harness`.

### Keep the no-flag placement default as current cwd

**Observation:** The old `--pool` concept becomes "where does bare `lf` run from inside a wave?" once `lf q worker` disappears.

**Cost:** Low. The resident already enters the wave worktree when that is the desired cwd.

**Benefit:** Bare `lf` stays literal: it runs where the shell is. Relocation is explicit.

**Verdict:** Current cwd is the M1 default. The explicit relocation APIs are `--dispatch`, `--stack`, and `--fork`; do not preserve `--pool` unless it is only a temporary alias.

### Remove `lf q worker` from the user-facing surface in M1

**Observation:** Keeping both `lf q worker run ...` and `lf <flow> --dispatch/--stack/--fork ...` would recreate the two-dispatch-worlds problem under a new spelling.

**Cost:** Medium-high. The resident prompt, CLI parser, dispatch tests, session registration, docs, and journal text all reference `lf q worker run`.

**Benefit:** Work placement becomes an execution option, not a separate API.

**Verdict:** Make this an M1 headline. Replacement placement flags should run synchronously in the placed worktree and block like normal `lf`; remove `QCommand`/`WorkerCommand` as a public command once replacement flags exist.
