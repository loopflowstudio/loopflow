# Goal primitive — tighten to the loop body (wave/goals item 1, follow-on)

Vision: `wave/goals/README.md`. Decisions + why: `scratch/goals-launch-plan.md`.
Release decisions: `release/unreleased/DECISIONS.md` (2026-06-30).

## Status

The goal primitive already landed: `Goal { prompt }`, `GoalRenderContext`,
`load_goal`, `render_goal`, and the wave-loop wiring in
`lfd/executor/wave/mod.rs::build_wave_run_command`. **Do not re-implement those.**
RLM was also already removed in this working tree (don't touch that either).

This unit *tightens* the primitive into the decided shape and ships two adjacent
cleanups. Three pieces, all decided, no open forks block them:

1. **`wave.goal` is never nil** — the goal is the unconditional loop body.
2. **`Surface::Ide`** — handoffs carry the task, not loopflow's voice/surface skin.
3. **One goal resolution path** — `.lf/goals/` only.

## What to build

### 1. `wave.goal` never nil

Looping is the essence of a Wave; a wave without a goal is invalid state. Make
`goal` a **required** field on the canonical record, default `"ship-roadmap"`.

The goal is the loop body **unconditionally**. There is **no** flow-override
fallback: do not gate the goal on `run.snapshot.flow == primary_flow`, do not keep
a flow-as-loop-body else-branch. Delete the else-branch in `build_wave_run_command`.

`primary_flow` **stays** as a field (the default flow the goal dispatches) — its
deletion is a separate later cut, out of scope here.

### 2. `Surface::Ide`

When loopflow hands work to another agent's surface (IDE deep-link), the seed must
carry the task only — not `<lf:voice>` and not `surface.instructions()`. The host
agent owns voice and interaction. Today `run.rs` picks `Surface::Cli` for any
interactive run before it knows the target is an IDE, so the voice+surface-baked
prompt gets percent-encoded into `claude://code/new?...&q=`.

### 3. One goal resolution path

`find_goal_path` currently hedges across five variants (`.lf/goals/`, `.lf/goal/`,
repo-root `goal/`, `~/.lf/goals/`, `~/.lf/goal/`). Collapse to **`.lf/goals/`**
(repo) and **`~/.lf/goals/`** (user) only. Delete singular + repo-root variants.

## Data structures (target state)

```rust
// rust/loopflow/src/lfd/types/wave.rs — canonical record
pub struct Wave {
    // ...
    pub goal: String,          // required; the goal this wave loops. Default "ship-roadmap".
    // primary_flow stays as-is (default flow the goal dispatches)
}

// rust/loopflow/src/lfd/http/dto.rs — wire DTO mirrored in Swift
// goal: String  (required — NO #[serde(default)], no Option)
```

**Record vs patch is load-bearing — do not blanket-replace `Option<String> goal`:**
the partial-update DTOs keep `Option<String>` (there `None` = "don't change this
field", legitimate optionality):
- `WaveConfigUpdate.goal: Option<String>` (stays)
- `RunOverrides.goal: Option<String>` (stays)
- the PATCH payload structs in `lfd/http/routes/waves.rs` (stay Optional)

Mirror in Swift: `swift/LoopflowCore/Models/Wave.swift` `goal: String` (required;
parse with no `as? String` nil-fallback). `LocalWaveService.swift` update/override
structs keep `String?`.

## Key seams

```rust
// lfd/executor/wave/mod.rs — goal is unconditional; delete the else-branch
fn build_wave_run_command(wave: &Wave, run: &WaveRun) -> Result<(Vec<String>, String)> {
    let goal = load_goal(wave.goal(), Path::new(&run.worktree))?;   // wave.goal() -> &str
    let prompt = render_goal(&goal, &GoalRenderContext {
        flows: available_flow_names(repo),
        roadmap: format!("wave/{}", wave.name()),
    });
    // ... build_lf_inline_command ...
    Ok((cmd, format!("goal:{}", wave.goal())))
}

// lfd/types/wave.rs
pub fn goal(&self) -> &str { &self.goal }     // was Option<&str>

// engine/prompt.rs — Surface gets an Ide variant
enum Surface { Headless, Cli, Ide, ConcertoMac, ConcertoIphone }
impl Surface {
    fn instructions(&self) -> &str { /* Ide => "" */ }
    // voice gate moves off is_interactive(): an Ide handoff IS interactive but is
    // NOT loopflow-driven. format_system_sections drops voice AND surface for Ide.
}

// lf/commands/run.rs — pick Ide up front so the prompt never bakes voice/surface
let surface = if cli.ide { Surface::Ide }
              else if is_interactive { Surface::Cli }
              else { Surface::Headless };

// engine/flow.rs::find_goal_path — only:
//   repo/.lf/goals/<name>.md, then ~/.lf/goals/<name>.md (+ namespaced prefix form)
```

Migration `037_wave_goal.sql`: backfill existing rows
(`UPDATE waves SET goal = 'ship-roadmap' WHERE goal IS NULL`) then enforce
`NOT NULL`. Sweep the ~40 `goal: None` literals in constructors/tests to
`"ship-roadmap".to_string()` (or the test's intent).

## Constraints

- **No flow-override fallback.** The goal is the loop body, always. No
  `.filter(|_| run.snapshot.flow == primary_flow)`, no resurrected else-branch.
- **Record strict, patches Optional.** Required `String` on `Wave`/wire DTO/Swift
  `Wave`; `Option`/`String?` on the update/override DTOs. (CLAUDE.md DTO rule.)
- **No compat shims.** Migrate the resolver to one path; don't accept legacy
  `.lf/goal/` or repo-root `goal/`. Backfill the DB, don't make the column nullable.
- **`primary_flow` survives** this unit untouched as the dispatch default.
- **Surface::Ide drops voice + surface**, keeps the task. `format_system_sections`
  is already rlm-free, so this is the only remaining gate.

## Done when

- `Wave.goal` is `String` (required) in Rust record, wire DTO, and Swift `Wave`;
  the three update/override DTOs remain Optional. Round-trip fixture
  (`tests/fixtures/wave.json` + per-language fixture tests) updated for required
  `goal`.
- `build_wave_run_command` always renders the goal; the flow-as-loop-body branch is
  gone. A smoke test runs one iteration of a `goal:`-set wave and asserts the goal
  prompt reaches the command.
- Migration backfills existing waves to `ship-roadmap` and the column is `NOT NULL`.
- `lf <step> --ide` produces a deep-link prompt with **no** `<lf:voice>` and **no**
  surface-instruction block; a non-IDE interactive run still includes them. Test
  asserts both.
- `find_goal_path` resolves only `.lf/goals/` (repo) and `~/.lf/goals/`; a goal in
  `.lf/goal/` or repo-root `goal/` no longer resolves. Override test updated.
- `cargo test` passes; `cargo fmt` + `cargo clippy -- -D warnings` clean.

## Not in scope (defer)

- `prepare_goal_launch` launcher + the LOOPFLOW operating prompt (next unit).
- `author-goal` step, `Target::Goal` discovery, `lf goal`/`create-wave` front doors.
- Deleting `primary_flow`.
- Persistence backend (A1/A2/tmux-backend-0).
- Map-reduce-as-step (RLM technique migration).
