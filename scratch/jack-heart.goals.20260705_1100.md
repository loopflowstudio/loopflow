# M1: Shape Loopflow Around the New Wave Architecture

## What to build

Land M1 as one ambitious PR: enforce the component charter from `wave/goals/architecture-direction.md` so the current demo architecture stops depending on accidental module ownership.

Jack's scope anchor: "M1 is itself part of a larger staged plan. one pr for m1."

M1 is the conversion/shaping milestone, not the substrate deletion milestone. It should leave the live wave-agent behavior intact while making the dependency graph say the same thing as the architecture:

- `lf` is the hands: thin verbs and argv-facing UX.
- `wave` is the listener/resident boundary: pens, channels, resident wire, supervisor, and mind lifecycle.
- `resident` owns vendor harness usage, but the harness code itself is not owned by `lfd`.
- `engine` owns material facts: repo root, wave file conventions, worktree naming, config, prompt/flow/goal loading.
- `placement` owns worktree selection for normal `lf` runs: same-target dispatch, stack, and fork. It may still record run/session facts, but placement is not a detached worker API.
- `lfd` is a local gatekeeper/query surface. It can read, push, listen, and exec `lf`; it must stop owning shared config, dispatch, harness, tmux placement, or git mutations.

User-facing direction from Jack: M1 removes `lf q worker run`. Work placement becomes flags on ordinary `lf` execution:

```bash
lf implement "task" --dispatch   # separate worktree, same remote target branch
lf implement "task" --stack X     # separate worktree, stacked on X
lf implement "task" --fork        # separate worktree, independent branch from HEAD's base
```

These flags change the cwd/worktree for the agent launched by this `lf` command. They do not detach by default; `lf` blocks on the agent as if it were running normally.

No placement flag means current cwd. Bare `lf implement "task"` runs exactly where the shell is, even inside a wave. The resident can still run from the wave worktree because it already enters that cwd before launching its agent.

## Current cuts to make

The repo already names most of the M1 debt in TODOs and imports:

```text
wave/resident.rs
  imports lf::commands::sub::stream_events
  imports lf::commands::util::find_repo_root
  imports lfd::conversations::harness
  imports lfd::executor::ensure_wave_worktree
  imports lfd::http::routes::wave_config::read_wave_config

wave/mod.rs
  imports lf::commands::util::find_repo_root

lf/commands/q.rs
  owns today's public dispatch API (`lf q worker run`)
  imports lfd::executor::{create_run_for_placement, Placement}
  imports lfd::executor::helpers::{tmux/dispatch helpers}
  should disappear as a public command after replacement placement flags land

lfd/http/routes/wave_config.rs
  owns GOAL.md frontmatter parsing and writing used by wave, ops, resident, lfd

lfd/conversations/
  owns harness/types/turns consumed by the resident and wave listener

lfd/executor/helpers.rs
  owns placement, wave/run worktree naming, tmux launch wiring, dispatch env
```

M1 is done when those ownership violations are gone or intentionally fenced as temporary debt with a narrower owner.

## Target module shape

Sketch the end state before coding. Names can shift if the local code wants a cleaner split, but the direction should not.

```rust
// rust/loopflow/src/engine/repo.rs
pub fn find_repo_root() -> anyhow::Result<PathBuf>;

// rust/loopflow/src/engine/wave_config.rs
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct WaveConfig {
    pub goal: Option<String>,
    pub primary_flow: Option<String>,
    pub mode: Option<String>,
    pub crons: Option<Vec<WaveCronDef>>,
    pub workers: Option<u32>,
    pub agent: Option<String>,
    pub step_agents: Option<HashMap<String, String>>,
    pub pm: Option<WavePmConfig>,
    pub mind: Option<String>,
    pub paused: Option<bool>,
}

pub fn read_wave_config(repo: &Path, name: &str) -> Option<WaveConfig>;
pub fn update_wave_goal_config(
    repo: &Path,
    name: &str,
    update: impl FnOnce(&mut Mapping) -> Result<(), String>,
) -> Result<(), String>;

// rust/loopflow/src/engine/worktrees.rs
pub fn ensure_wave_worktree(main_repo: &Path, wave_name: &str) -> anyhow::Result<WorktreeLease>;
pub fn run_worktree_path(main_repo: &Path, wave_name: &str, run_id: &str) -> PathBuf;
pub fn short_run_id(run_id: &str) -> String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeLease {
    pub path: PathBuf,
    pub branch: String,
}
```

```rust
// rust/loopflow/src/engine/placement.rs, or a sibling module with the same role
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    Current,
    DispatchSameTarget,
    Stack { target: StackTarget },
    Fork,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackTarget {
    Run(LfdId),
    Branch(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedRun {
    pub repo_root: PathBuf,
    pub worktree: PathBuf,
    pub branch: String,
    pub target_branch: String,
    pub channel: Option<String>,
    pub stack: Option<StackLineage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackLineage {
    pub parent_branch: String,
    pub parent_run_id: Option<LfdId>,
    pub parent_pr_number: Option<u32>,
    pub inferred: bool,
}

pub async fn resolve_placement(
    origin_repo: &Path,
    requested: Placement,
) -> anyhow::Result<PlacedRun>;
```

Placement flags feed normal `lf` launch. They do not spawn tmux or return early. `lf::commands::run::build_prompt` should resolve placement before context assembly and pass the placed worktree as `repo_root`/cwd to prompt launch.

`--dispatch` means separate worktree, same remote target branch. Because git cannot check out the same local branch in two worktrees, implementation likely needs a run-local local branch that pushes back to the current remote branch, or a detached worktree that pushes `HEAD:<target>`.

`--stack X` means separate worktree, new branch stacked on X. Stack truth should be portable: git ancestry first, PR base second, lfdb lineage as annotation/cache.

`--fork` means separate worktree, independent branch from this branch's review base: normally `origin/<default>`, except when the current branch is stacked, where the base is the branch it is stacked onto.

```rust
// rust/loopflow/src/wave/events.rs or wave/subscription.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub event: String,
    pub data: String,
}

pub struct SseFrameParser { ... }

pub async fn stream_events(
    endpoint: &str,
    query: &str,
    on_frame: &mut impl FnMut(Frame),
) -> anyhow::Result<()>;
```

`lf sub` should render this component. The resident should subscribe through it. `wave` must not import command code.

```rust
// rust/loopflow/src/harness/
pub trait Harness { ... }
pub fn default_create_harness(...) -> anyhow::Result<Box<dyn Harness>>;
pub fn canonical_harness(name: &str) -> Option<&'static str>;

// Existing lfd::conversations::turns/types need a deliberate home:
// either crate::harness::{types, turns} for now, or wave-owned turn vocabulary
// if moving them is cheap in the same PR.
```

`lfd::conversations` should not remain the owner of the resident's harness dependencies. If moving `turns` creates churn in Swift/DTO fixture mirrors, keep the public Rust type shape stable and move module paths only inside Rust.

## Implementation sequence

1. Move repo-root and wave config into `engine`.

   Replace imports of `crate::lf::commands::util::find_repo_root` and `crate::lfd::http::routes::wave_config::*` with `crate::engine::*` APIs. Keep `lf/commands/util.rs` as a thin shim only if many command call sites make a same-PR sweep too noisy; otherwise delete the duplicated function.

2. Make worktree naming single-source.

   Move `ensure_wave_worktree`, `run_worktree_path`, `short_run_id`, and run-worktree creation rules out of `lfd/executor/helpers.rs`. The existing `engine/worktrees.rs` already owns sibling naming; extend it rather than inventing another naming module. Dispatch and resident should both call this owner.

3. Replace `lf q worker run` with placement flags on normal `lf`.

   Add top-level placement flags for ordinary agent-launching runs:

   ```bash
   lf implement "task" --dispatch
   lf implement "task" --stack <run-id-or-branch>
   lf implement "task" --fork
   ```

   These flags change the worktree for the agent launched by the current `lf` invocation and block until the agent exits. Do not recreate the old detached worker API under new names.

   Remove `Commands::Q`, `QCommand`, and `WorkerCommand` from the public CLI once the placement flags cover the behavior. Keep only the narrower helpers that placement still needs: worktree creation, run/session annotation if required, channel naming, and queue metadata.

   After extraction, `lfd::executor` should not own placement, wave/run worktree naming, or worker launch wiring. If a small compatibility surface remains for palette/session cleanup, name it around that narrower job.

4. Move the SSE client out of `lf`.

   `lf sub` keeps CLI rendering. The reusable parser/client becomes a wave-owned or shared component consumed by both `lf sub` and `wave::resident::follow_inbox`.

5. Move harness out of `lfd`.

   Create `crate::harness` and re-home `lfd/conversations/harness`. Move `types` and `turns` only as far as needed to break ownership confusion. The resident imports `crate::harness`, not `crate::lfd::conversations`.

6. Make `lfd` hand routes exec `lf` instead of mutating in process.

   Routes like `/land`, `/next`, `/combine`, `/stop`, and rename should plan and execute an `lf` argv under the local capability-token authority instead of calling ops/tmux/git directly. Preserve the security mechanics called out in `architecture-direction.md`: bearer parsing, query-token rejection, throttling, local token files, signature verification, redaction, and replay behavior.

   Keep the existing Concerto-facing response shapes unless Jack chooses a UI/API break: Swift still calls these routes and expects `LandWaveResponse`, `NextWaveResponse`, `CombineResponse`, and `StopWaveResponse`.

7. Sweep vocabulary and grammar.

   Do the `step` to `skill` user-facing sweep only if it still fits after the placement work. Preserve old internal names where changing them would explode the PR without changing behavior. Ensure command grammar reads as the intended surface: `skill | flow | op | :`, with placement flags on ordinary `lf` runs.

8. Delete dead compatibility only when it reduces architecture pressure.

   Strong candidates from `wave/goals/wave-agent-follow-ups.md`: public `lf q worker` command, old goal-agent launch path, `roadmap_item` plumbing, and any leftover trigger/activation references that survived migration cleanup. Do not delete postgres/container/dual-backend machinery here; that is M2.

   Research correction: the old lfd HTTP worker route appears already gone from current code; `lf q worker run` is the active dispatch path today, but M1 should remove it.

## Dependency invariants

M1 should make these checks true in spirit and, where practical, true by `rg`:

```text
wave -> lf::commands             forbidden
wave -> lfd::http::routes        forbidden
wave -> lfd::executor            forbidden
wave -> lfd::conversations       forbidden after harness move

lf -> lfd::executor              forbidden for placement paths
lf -> placement/engine           allowed
lf -> engine                     allowed
lf -> wave subscription client   allowed for lf sub

lfd::http::routes::wave_config   gone or shim only
lfd::conversations::harness      gone or shim only
lfd::executor::helpers           no longer owns placement/worktree naming
```

The more important rule: commands may import components; components must not import commands.

## Non-goals

- Do not delete postgres, container mode, or dual-backend machinery. M2 owns substrate deletion.
- Do not design remote identity/auth. M3 owns `lfq` as HTTP proxy and client identity.
- Do not introduce a central daemon dependency for local work. Every local capability still needs a daemonless path.
- Do not create backwards-compatible roadmap mirrors in `wave/`. Asana remains the roadmap source of truth.
- Do not reshape production code solely for tests. Use focused unit tests around moved pure functions and smoke tests around behavior.

## Tests and verification

Run the smallest fast checks after each move-set, then the full Rust gate before handing back:

```bash
cargo fmt
cargo test -p loopflow
cargo clippy -- -D warnings
```

Add or preserve focused tests for:

- `engine::wave_config::read_wave_config` and `update_wave_goal_config`.
- worktree path derivation and short run ids.
- placement behavior for `--dispatch`, `--stack`, and `--fork`.
- stack/fork base resolution: PR base/git ancestry first, lfdb metadata as annotation.
- run/session env contract for placed runs, including `LFD_CHANNEL` where channel context applies.
- SSE parser replay/live frame behavior after moving out of `lf sub`.
- harness conformance traces still passing after the module move.

Add a dependency-direction smoke check if cheap:

```bash
rg -n "crate::lf::commands|crate::lfd::http::routes::wave_config|crate::lfd::executor|crate::lfd::conversations" rust/loopflow/src/wave
rg -n "crate::lfd::executor" rust/loopflow/src/lf rust/loopflow/src/wave
```

Expected result: no forbidden imports, except any documented shim that exists only to keep the PR reviewable.

## Done when

- The live two-process wave demo still runs: `lf wave goals` starts listener + resident, and ordinary `lf <flow-or-step> ... --dispatch/--stack/--fork` placement runs report back through the channel family.
- `cargo fmt`, `cargo test -p loopflow`, and `cargo clippy -- -D warnings` pass.
- `wave/goals/architecture-direction.md` M1 known-debt bullets are either resolved or narrowed to explicit follow-ups.
- The dependency graph follows the charter: `lfd` no longer owns shared wave config, placement, harness, or worktree naming.
- The PR description can explain M1 in one screen: "commands call components; components no longer call commands; lfd is back to gatekeeper/query surface."

## Open questions for Jack

1. Should `--dispatch` use a run-local branch that pushes to the same remote target, or detached HEAD with explicit `HEAD:<target>` pushes?

   Git cannot check out the same local branch twice. The design requires separate worktree, same remote target branch; implementation must choose the safest git mechanism.

2. Should lfd hand routes preserve their current Swift DTOs while internally execing `lf`, or is this PR allowed to change Concerto to a new command/result model?

3. Is `step` to `skill` a full user-facing rename in this PR, or should M1 only establish the grammar and leave file/struct/path renames for a follow-up?

   A full rename reaches `Step`, `FlowItem::Step`, `.lf/steps`, builtin `*/step/` directories, `<lf:step>` tags, Python `FlowStep`, Swift `Step` models, and golden prompt fixtures.

4. How aggressive should the old goal-agent launch/render path cleanup be? `wave::mind` still uses `render_goal` for the resident seed; `engine::prompt` also has wave-agent inline-run logic.

5. For turn vocabulary, is the preferred home `crate::harness::{types, turns}` for now, or should turns move under `wave` because the listener streams them?

6. Should the PR update `wave/goals/architecture-direction.md` after implementation to mark M1 debt resolved, or should that durable doc stay as the target snapshot and the PR body carry the status?
