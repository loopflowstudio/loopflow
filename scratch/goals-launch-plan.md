# Goals — launch plan & design decisions

Companion to `scratch/jack-heart.wave-looping-agents.md` (the goal-primitive PR,
already built) and `wave/goals/README.md` (the chord vision). This doc captures
the decisions made in the design/demo session of 2026-06-30 that go *beyond* the
landed primitive: how goals get launched, how the loop is surfaced, and a few
data-model corrections. Decisions are marked **DECIDED** or **OPEN**.

## Where the primitive stands

`Goal { prompt }`, `GoalRenderContext { flows, roadmap }`, `load_goal`,
`render_goal` (emits `<lf:goal-context>`) are landed in
`rust/loopflow/src/engine/flow.rs`. The wave loop is wired:
`lfd/executor/wave/mod.rs::build_wave_run_command` reads `wave.goal()`, loads,
renders with `flows = available_flow_names(repo)` and `roadmap = "wave/<name>"`,
and runs it as the iteration body (returns label `goal:<name>`). Override + render
tests pass.

Naming/error note (intentional drift from the PR plan): resolution landed as
`load_goal -> Result<Goal, LoadError>`, not the plan's `find_goal -> Option`. A
wave naming an unresolvable goal is a config error the user must see, not a silent
`None`. Backed by commit "report missing goals clearly." Keep `Result`.

## DECIDED

### 1. `wave.goal` is never nil — looping is the essence
A Wave *is* "one looping unit of work"; a wave without a goal is a contradiction.
- `Wave.goal: Option<String>` → **required `String`**, default `"ship-roadmap"`,
  on the canonical record (`lfd/types/wave.rs`, wire DTO `lfd/http/dto.rs`, Swift
  `Wave`). This is *more* DTO-compliant: an absent required field is a parse error,
  not a silent `None` (kills the `json["goal"] as? String` drift).
- **Record vs patch split is load-bearing:** the partial-update DTOs
  (`WaveConfigUpdate`, `RunOverrides`, the PATCH payloads at `waves.rs:103/119/135`)
  **keep `Option<String>`** — there `None` means "don't change this field," which
  is legitimate optionality (mirrors how `primary_flow` the record-field is required
  while `WaveConfigUpdate.flow` is Optional). Do *not* blanket-replace every
  `Option<String> goal`.
- The `if let Some(goal_name) = wave.goal()` branch in `build_wave_run_command`
  goes **unconditional**; the flow-as-loop-body else-branch is dead code → delete.
- Migration `037_wave_goal.sql` (`ADD COLUMN goal TEXT`) becomes backfill existing
  rows to `"ship-roadmap"` then `NOT NULL`.
- ~40 `goal: None` literals across constructors/tests → `"ship-roadmap".to_string()`
  (or the test's intent).

### 2. The operating prompt lives in the launch system layer, not `render_goal`
The universal "you are a looping orchestrator; delegate, don't hand-write code;
your three powers are read-roadmap-and-metrics / dispatch-flows / spawn-child-waves"
contract belongs in the **launch system_prompt**, authored once, applied to every
looping session. `render_goal` stays **pure** (the goal's prose + context handles,
no preaching). Split: `render_goal` = the task this iteration; operating prompt =
who you are across all iterations. This is the README's "two layers in the seed."
The operating prompt is the one piece of net-new prose the launcher needs.

### 3. Launcher / creator split — the launcher is the keystone
Build and name **one** reusable launcher; every front door is a thin caller.
- Launcher = goal analog of `engine/launch.rs::prepare_launch_prompt`. Composes:
  `system_prompt = operating prompt`, `task_prompt = render_goal(goal, ctx)`, then
  starts an interactive agent session. Call it `prepare_goal_launch`.
- Front doors (all call the launcher): `lf goal`/`lf create-wave` (author new),
  `lf <name>` (launch existing), the Concerto button.

### 4. `.lf/goals/` is the one canonical goal directory
`find_goal_path` currently hedges across **five** variants (`.lf/goals/`,
`.lf/goal/`, repo-root `goal/`, `~/.lf/goals/`, `~/.lf/goal/`) — the "handles both
old and new" smell CLAUDE.md forbids. Collapse to **`.lf/goals/`** only (plural,
matches `.lf/steps/` and `.lf/flows/` — one rule). Delete the singular and
repo-root variants. The launcher saves there; the resolver reads only there.

### 5. Authoring a goal is a step (models on `kickoff`)
`lf goal` (bare) = author + launch. Phase 1 runs a builtin **`author-goal`** step
(interactive one-shot, same shape as `kickoff`: fuzzy intent → written artifact):
interviews the user, co-writes goal prose + success **metrics in the prose** (not a
struct field) + picks a default flow, lands `.lf/goals/<name>.md`. Phase 2 calls
the launcher on that goal. No bespoke wizard code — it's a step, overridable.

### 6. First cut launches ONE watchable interactive session
Phase 2 launches the agent for a *single* orchestration turn (read roadmap → decide
→ dispatch a flow) that you watch — **not** the persistent 24/7 loop. This is the
demo; it proves the seed steers an agent and defers the persistence decision. Teach
`discover_target` a third `Target::Goal` so `lf <name>` launches an existing goal
for free (no authoring). The looping launch becomes `lf goal --loop` / Concerto
button later.

### 7. Embedded tmux sessions are the primary surface; IDE is demoted
tmux is already the session substrate (`tmux_session_name`, `tmux_session_exists`,
`disable_tmux`; Concerto attaches via the terminal-sessions route +
`MultiplexerView`/`TerminalWorkspaceView`). So:
- **No bespoke "head dashboard" needed.** The head is the agent's live transcript
  in an embedded tmux pane. `lf goal` (local terminal) and Concerto (embedded) are
  two viewers of one tmux session.
- `session.launch: cli` is the default to design around; `lf ide` / Warp / Cursor
  stay supported but secondary.
- **tmux gives cheap local session persistence** (detached survives Concerto + lfd
  restart) — a possible "backend (0)" below vendor-cloud (a) and hosted-lfd (b).
  Caveat: tmux persists the *session*, not the *loop*; the loop driver
  (ticker/cron/self-loop) is orthogonal. Don't conflate.

### 8. Handoffs carry the task, not loopflow's session skin (`Surface::Ide`)
When loopflow hands work to another agent's surface (IDE today, vendor-cloud later),
`<lf:voice>` and `surface.instructions()` are noise — the host owns voice and
interaction. **Bug today:** `run.rs:113` picks `Surface::Cli` whenever interactive,
before `launch_prompt` learns it's an IDE handoff, so the whole voice+surface-baked
prompt gets percent-encoded into `claude://code/new?...&q=`.
- **Fix:** add `Surface::Ide`, selected up-front when `cli.ide`. Its
  `instructions()` is empty and it drops voice — the real bug is that voice is gated
  on `is_interactive()` when it should be gated on "loopflow-driven" (an IDE handoff
  *is* interactive, just not loopflow-driven). With `<lf:rlm>` now removed,
  `format_system_sections` is down to voice + surface, so the Ide gate is ~2 lines.
- Same principle the launcher uses (operating prompt, not generic skin) and what the
  vendor-cloud handoff (backend a) will reuse. **Not yet implemented.**

### 9. RLM removed (DONE — see DECISIONS.md 2026-06-30)
The recursive-LM map-reduce framework + its always-on `<lf:rlm>` injection + config
+ depth-guard machinery are deleted. Goals supersede it as the subagent-running
model; runaway stop-condition is goals' blocks→human, not `RLM_MAX_DEPTH`. The
map-reduce *technique* may return as an invokable step if wanted.

## OPEN — resolve at build time

- **`primary_flow`'s fate.** Goal-never-nil makes the goal the loop body, so
  `primary_flow` is no longer "the wave's loop." Keep it as "the default flow the
  goal dispatches" (minimal; recommended now) or delete it (cleaner model, but a
  column drop + DTO removal across three languages, orphans existing waves). Lean:
  keep now, kill as a separable follow-up — `ship-roadmap`'s prose already says
  "pick a flow and dispatch it," which makes `primary_flow` feel redundant.
- **`lf goal` vs `lf create-wave`.** Same launcher underneath; the difference is
  whether the front door also writes a persistent Wave row + recurring trigger.
  Resolves with #6 (session-first) and the backend choice below.
- **Persistence backend: A1 vs A2** (`wave/goals/2-looping-agent-cloud.md`). README
  leans **A2** (scaffold + human presses go; vendor's own recurring trigger;
  "we rent persistence"). A1 (lfd drives the cloud API + trigger) makes Concerto a
  real dashboard but couples to moving vendor APIs. Plus the tmux "backend (0)"
  option from #7.
- **Map-reduce technique:** migrate `RLM.md` content to `.lf/steps/map-reduce.md`
  (invoke when needed) or accept losing the explicit playbook. Lean: migrate to a
  step.

## Immediate next build unit (this is what `lf code` should pick up)

Concrete, fully decided, no open forks blocking:
1. **`Surface::Ide`** handoff fix (#8) — select it when `cli.ide`; empty
   instructions; drop voice. ~2-line gate in `format_system_sections` now that rlm
   is gone, plus surface selection in `run.rs`.
2. **`wave.goal` never nil** (#1) — required field on the record (keep patch DTOs
   Optional), default `ship-roadmap`, delete the dead else-branch, migration
   backfill + `NOT NULL`, sweep the `goal: None` literals. Keep `primary_flow` for
   now.
3. **Collapse `find_goal_path` to `.lf/goals/` only** (#4).

Larger, design-settled but more surface (subsequent units): `prepare_goal_launch` +
the operating prompt (#2/#3), the `author-goal` step (#5), `Target::Goal` discovery
(#6).
