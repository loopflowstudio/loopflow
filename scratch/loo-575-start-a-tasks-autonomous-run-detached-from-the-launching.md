# LOO-575: `lf task run` returns once its Graph is running

Status: draft plan (kickoff, 2026-10-24). Jack Heart filed the Task on 2026-10-24
for a fully autonomous run; the command shape below is an executive choice
recorded in `scratch/questions.md`, open to demo review.

## Problem

On 2026-10-24 an agent launched LOO-573 with `lf -b task run LOO-573 autonomous -a claude`
expecting it to detach. `-b` means "return when the work ends", so the launcher
held the conversation's tool for the whole Graph and its shell kept it as a job.
The only launcher-independent recipe is `nohup lf -b task run … </dev/null >log 2>&1 &`,
which lives in personal memory notes, not in lf. Skills tell agents
"`task run` returns when its Graph ends; run it with your own background tool".

## Demo

```text
$ lf task run LOO-575 autonomous
LOO-575 · autonomous running as Graph process 8985247f-…
  follow: lf task status LOO-575
  log:    ~/.lf/logs/graph-8985247f-….log
$ exit                                  # or close the conversation
$ lf task status LOO-575
  Graph: 8985247f-…  autonomous  Current  log: ~/.lf/logs/graph-8985247f-….log
```

The prompt comes back in seconds; closing the launching shell or conversation
leaves the Graph advancing; `lf top` and Desktop show it as before.

## Risk findings (observed 2026-10-24)

- **The owner is already detached; the launcher is not.** `delegate_current`
  (`transport.rs`) starts `__transport-serve` with `setsid` (own session, no
  controlling terminal, stderr to `logs/transport.log`), then the launcher stays
  attached as a viewer until the run ends. Live `ps` for LOO-573: launcher 20293
  in the Claude shell's group 20289; owner 20382 in its own group 20382 but
  PPID 20293; inner `lf -b task run` 20517 in group 20517. So a process-group kill
  of the launching shell already spares the owner. What breaks is (a) the
  launcher blocks for the whole Graph, and (b) while it lives, the owner is a
  PPID-descendant of the conversation, so a tree-walking kill (as agent tools do
  when cleaning background jobs — plausible, not reproduced) takes it too. Once
  the launcher exits the owner reparents to launchd (seen for the LOO-561 owners).
- **Owners outlive viewers.** A viewer disconnect only drops it from `viewers`.
  The idle Session policy (`transport/idle.rs`, `idle_session_owners`) excludes
  Graph ancestors, so an unviewed Graph is never ended at 24 h.
- **The Graph process ID appears only after placement.** The owner runs
  `lf task run` (inner), which places the Task, then `run_for_task` spawns the
  `lf graph` child — the Graph process — and retries it up to `TASK_RUN_ATTEMPTS`.
  The launcher cannot name the Graph at owner acknowledgement time.
- **Unviewed output is discarded.** With no viewer, the inner process's relayed
  output goes nowhere durable; `terminal-<id>.log` exists only when stdout is a TTY.
- **The detach capability exists.** `transport::detach_viewers()` lets the owned
  execution (control key) end every viewer with `Response::Detached`, after which
  `attach` returns `Ok`. Used today by `/detach` in `ops/task_input.rs`.
  Ordering hazard: `serve` handles `DetachViewers` before draining child output in
  the same loop pass, so a line printed just before detaching can miss the viewer.
- **Steps are already headless** without `-b` (`Cli::step_args` passes `--batch`
  unless `-i`), so bare `lf task run` has no interactive-step trap left.
- **Desktop has its own approximation.** `LocalWaveAgentLauncher.startLf` spawns
  `lf -b task run` as an app child and treats "still running after 3 s" as started,
  with output in a temp file. Its finite runner (`run`) has a 30 s deadline that
  kills only the direct child, never an execution owner.

## Decisions

- **Bare `lf task run` starts and returns; `-b` keeps waiting.** (2026-10-24,
  executive; questions.md.) Without `-b`/`-i`, `lf task run` returns once its
  Graph process is registered. `-b` keeps its documented meaning (scripts, CI,
  fixtures); `-i` stays attached because
  its steps need the terminal. No new flag: the obvious command is the one the
  docs already show (`lf task run INF-123 pursue`), and Workflow edges are always
  autonomous.
- **The inner run decides when to let go, through the existing transport.**
  No second runner, no double fork, no new launcher. Placement output and
  refusals still stream to the launcher; it returns non-zero with the real error
  when placement fails or the Graph exits before registering. No elapsed-time cutoff.
- **One log per Graph process**: `<LF_HOME>/logs/graph-<graph process id>.log`,
  written only for runs that let go. The name derives from the Graph process ID,
  so no store column or DTO field is added.

## Design

Launch path, unchanged up to the inner process: launcher → `delegate_current`
→ `setsid` owner → inner `lf task run` → `task_place` → `run_for_task`.

`run_for_task(cli, …)` gains a mode `letting_go = !cli.batch && !cli.interactive
&& transport::current().is_some()` (owned executions only; an unowned call stays
foreground). Per attempt, when letting go:

1. Open `logs/graph-pending-<inner id>-<attempt>.log` (0600, `O_NOFOLLOW`, like
   `TerminalOutput::enter`) and spawn the Graph child with stdout/stderr on it;
   skip `TerminalOutput::for_child`.
2. Poll `store.child_process_after(&process, mark)` (50 ms) until the child
   registers, or it exits first.
3. Exits first: copy the pending log to stderr, remove it, and follow the existing
   retry/exit rules — the launcher is still attached and sees the refusal.
4. Registered: rename the file to `graph-<id>.log` (the open descriptor follows).
   On attempt 1 print the demo's three lines to stdout, flush, call
   `transport::detach_viewers()`, then `dup2` the inner process's stdout/stderr
   onto the log so retry notices and `workflow_arrive` errors are retained.
   Later attempts `dup2` onto their own log before spawning.
5. Wait for the child and keep the existing retry and arrival behaviour.

`transport::serve`: move the `if detach { … }` block below the reader drain so
output written before `DetachViewers` reaches viewers before `Detached`.

`lf task status` text: append `  log: <path>` to a `Graph:` line when
`logs/graph-<id>.log` exists. JSON is unchanged.

Desktop: `runTask` runs `["task", "run", issue, graph?]` through the finite
runner; `resumeInterrupted` runs `["resume", "--all"]` the same way (it already
returns after starting owners). Delete the starter plumbing (below).

Guidance: replace "returns when its Graph ends; run it with your own background
tool" with "returns once its Graph is running; `lf task status <issue>` follows it;
`-b` waits for the end" in `builtins/task/skill/launch-plan.md`,
`builtins/task/skill/task_operate.md`, and check `advance.md`, `wave_operate.md`,
`s1.md`, `ops/skill/work.md`, `ops/skill/init.md`. Update `README.md` (Delegate),
`docs/agent-api.md`, `docs/lf.md`, `docs/getting-started.md` (`lf -b task run <issue-id> &`
→ `lf task run <issue-id>`), the `TaskCommand::Run` doc comment, and regenerate
`docs/lf-reference.md`.

## Contract

- **User-visible outcome**: people and agents start Task work with
  `lf task run <issue> [graph]`, get the Task, Graph process ID, follow command
  and log within seconds, and can close the launching shell.
- **Source of truth**: the Graph process's `processes`/`graph_processes` rows;
  the log file is output only, its path derived from the Graph process ID.
- **Affected consumers**: `lf task run` (bare), `transport::serve` ordering,
  `lf task status` text, Desktop `runTask`/`resumeInterrupted`, skills and docs.
  `lf -b task run` callers (tests, `world` fixtures) are unchanged. Graph `cmd:`
  steps run their authored argv without `--batch`, so a future `cmd: task run X`
  step would start X and finish; no repository or builtin Graph has one today.
- **Absent/error states**: unknown Task, refused placement, edge into `end` with
  no Graph (prints its existing line and returns), or a Graph child that exits
  before registering → launcher prints the error and exits with its code. Log
  absent → status prints no `log:`. Store unreadable while polling → error to the
  attached launcher; the child keeps running and the error says so.
- **Operational boundary**: returns after placement plus Graph registration
  (seconds; Desktop's 30 s runner deadline kills only its launcher, leaving the
  run going). No added timeouts.
- **Exclusions**: taskless `lf graph`/`lf skill` detach; `--pool`/`--rent`
  remote placement (remote `lf` receives the same argv); log rotation beyond
  `disk.rs`'s existing log pruning; JSON/DTO log fields; Ctrl+] behaviour.

## Delete — do not maintain

- `LocalWaveAgentLauncher.startLf`, `startRefusalWindow` and the temp
  `loopflow-start-*.log` files (`swift/LoopflowMac/MacLocalWaveAgentLauncher.swift`);
  `taskRunArguments`' `-b`.
- `RegistryStarter`, `RegistryQuery.start` and its `onMachine` forwarding
  (`swift/Loopflow/Services/RegistryQuery.swift`), the `start:` closure in
  `RegistryQueryLocal.swift`; move `DesktopHeadlessTests` assertions from
  `started.calls` to the runner's recorded calls with `["task","run",…]`.
- Guidance telling agents to background `task run` themselves (the skill lines above).

Forbidden: a `--detach` flag beside the new default, a second spawn path for
detached runs, a store column for the log, polling with a deadline, keeping
Desktop's 3-second heuristic as a fallback.

## Internal slices

One change. **This slice** first: `run_for_task` let-go + `serve` ordering + the
acceptance test below. Then status `log:`, guidance/docs/reference, Desktop cutover.

## Done when (gate, headless)

1. `cargo test -p loopflow --test integration task_run_returns_while_its_graph_keeps_running < /dev/null`
   (new, in `tests/linear/task_graph_launch_tests.rs`): a fixture Graph whose
   first step blocks on a release file (prior art: `__telemetry-scorecard` +
   script in `ops/task/lifecycle_tests.rs`) and whose second is a `cmd:` step.
   From `/bin/sh -c 'lf task run INF-123 held; touch $LF_HOME/returned; sleep 600'`
   in its own process group: `returned` appears while step 1 is still blocked;
   stdout names `Graph process <id>` and `graph-<id>.log`; `kill(-pgid, SIGKILL)`;
   write the release; the Graph records its second step Process and succeeds;
   the Task arrives at the edge target; `lf task status INF-123` prints
   `Graph: <id>` with `log: …graph-<id>.log`, and the log holds step output.
2. Same file: bare `lf task run` for a refused placement exits non-zero with the
   refusal and records no Graph process.
3. Existing `-b` suites stay green: `every_task_launch_runs_in_the_foreground_under_the_same_checks`,
   `graph_steps_run_headless_unless_asked_for_a_terminal`, `reference_matches_the_compiled_cli`.
4. Desktop: `DesktopHeadlessTests` (headless Swift) assert `runTask` issues
   `["task","run",id,graph]` through the runner and no starter exists.
5. `rg -n "returns when its Graph ends|nohup lf|-b task run <issue-id> &" rust/loopflow/src/builtins docs README.md`
   → no matches.

Demo (review): from a Claude Code conversation, `lf task run <issue> autonomous`,
close the conversation, watch `lf top` / Desktop show the Graph advancing.

## Checks

- kickoff: no build; evidence from code reading and live `ps` only.
