# LOO-974: managed Tasks on OpenCode and Pi

## Outcome

A Task whose step resolves to OpenCode or Pi runs like one on Claude or Codex:
`lf task run` starts the step, the step gets context and steers, runs tools,
records its answer, usage and native thread, and the Graph advances. A choice
that cannot run on this Machine fails once, with the reason, before any retry.

## Findings that shape the design

**The "execution boundary" denies nothing.** `AgentExecutionBoundary.writable_roots`
is built in `checkout_execution_boundary` (`rust/loopflow-execution/src/agent/mod.rs:105`)
and read by no launcher. With a boundary present Claude gets
`--dangerously-skip-permissions` (`agent/mod.rs:729`), Codex gets
`danger-full-access` with no writable-roots config (`agent/mod.rs:617`; pinned by
`managed_task_scope_carries_exact_delivery_capabilities`, `agent/mod.rs:1669`), and
Pi's argv is unchanged (`harness/pi.rs:141`). `docs/security.md` and
`docs/config.md:981` say the same: the boundary is the OS environment the caller
owns. The brief's "writes elsewhere are denied" is not true of any harness today.

**OpenCode is refused by a bool, not by a missing mechanism.**
`supports_execution_boundary: false` (`harness/opencode.rs:1393`) trips
`checkout_execution_boundary`, called for every headless step in a Task checkout
(`rust/loopflow/src/lf/commands/run.rs:635`), batch resume
(`agent/session.rs:132`) and Task status (`ops/task.rs:2299`). OpenCode's
`skip_permissions` already writes allow-all permissions (`opencode.rs:1277`),
the same contract Claude and Codex run under.

**Worktree scope without a boundary is production-dead.** Both production
writers set `write_scope = Worktree` together with a boundary (`run.rs:636`,
`agent/session.rs:127`). The provider-only branches (Claude strict sandbox
settings `agent/mod.rs:735`, Codex forced `workspace-write` `agent/mod.rs:611`,
OpenCode `external_directory` deny `opencode.rs:1272`, Pi's `ensure!`
`pi.rs:142`) are reached only by `launch_matrix.rs:159` and a replay fixture
(`rust/loopflow/src/lf/commands/replay.rs:232`).

**Three attempts come from one blind loop.** `TASK_RUN_ATTEMPTS = 3`
(`rust/loopflow/src/lf/commands/graph.rs:147`, loop 209-307) retries any Graph
child that exits 1. It stops early only for an interrupt, retained edits or a
limit row. `task_configuration_refusal` (`ops/task.rs:2283`) is called with
`skill: None` only, so a step's own agent is never checked before launch, and
nothing writes `Failed{resumable:false}`.

**No candidate is filtered.** `Config::agent_order` / `resolve_agents`
(`rust/loopflow-execution/src/config.rs:499,515`) and `_fallthrough_rungs`
(`agent/mod.rs:1171`) consult neither capability nor installed binary. `_climb`
(`agent/mod.rs:1207`) carries the first agent's boundary onto the next rung, so a
Codex step that falls back to OpenCode already runs today, unrefused.

**No test runs a worker on OpenCode or Pi.** Every `task run` integration test
uses Claude or Codex. The OpenCode and Pi Graph tests use `graph … --batch`
(`LF_GRAPH_STEP=bare`, `graph.rs:851`), which is a batch. The launch matrix never
sets a boundary, so its OpenCode Worker cell passes by skipping the refusal.

**A headless OpenCode question hangs.** A worker does not answer `form.created`
(`an_opencode_question_waits_until_it_is_answered`, `harness/fact_writer.rs:634`);
Pi fails fast (`harness/pi.rs:323`).

**Installed here:** OpenCode 2.2.1 lists `opencode/gemini-4-carbon`; Pi 1.3.0
lists Anthropic models. Both can serve the real acceptance run.

**Open PRs:** #1974, #1976 and #1972 touch `agent/mod.rs`, the harness files and
`ops/task.rs`, but none changes a line naming `execution_boundary`, `write_scope`,
`supports_execution_boundary` or `task_configuration_refusal` (grepped their
diffs against the merge base, 2026-11-18). #1946 touches neither.

## Design

One concept replaces three fields. A headless step in its Task checkout is
*unattended*: vendor prompts are bypassed and the OS environment is the limit.
That is `skip_permissions = true`, which every harness already understands.
`AgentWriteScope`, `AgentExecutionBoundary`, `supports_execution_boundary` and
`checkout_execution_boundary` go. With no capability to lack, no harness can
mismatch, and OpenCode needs no adapter work to be admitted.

Made for review, with what was passed over:

- *Passed over: flip OpenCode's bool to `true`.* Leaves a capability all four
  harnesses have and a descriptor nobody reads.
- *Passed over: make OpenCode enforce the roots* (`external_directory` deny plus
  the Git and `~/.lf` roots). It would be the only harness that denies, at tool
  level only, and Task steps would fail there on reads Claude and Codex allow.
- *Passed over: a real OS sandbox for all four.* A new feature that reverses
  `docs/security.md`; see `scratch/questions.md`.

What remains deterministic is "this harness is not installed". The agent order
drops candidates whose CLI is missing before choosing; a pinned `agent` that is
missing fails with the harness's `install_hint`. `lf task run` does not start
attempt two when the failed step never started a provider: the store already
knows whether the step's Process has an AgentProcess, so the rule is derived,
not a new failure type.

## Acceptance at gate

```bash
cargo test -p loopflow --lib task_graph_launch_tests::a_task_step_runs_as_a_worker_on_every_harness
cargo test -p loopflow --lib task_graph_launch_tests::a_step_that_cannot_launch_is_not_started_again
cargo test -p loopflow-execution --lib harness::launch_matrix
```

The first runs `task run` through `implement` with the Claude, Codex, OpenCode
and Pi stand-ins as workers (not `--batch`) and reads back one settled step, an
answer, a recorded native thread and the Graph at its next step. The second pins
an uninstalled harness and expects one attempt and the install hint.

Run by a person or at demo, on a throwaway Task with the branch `lf`:

```bash
lf task run <ID> -a opencode:opencode/gemini-4-carbon    # implement settles, Graph advances
lf task run <ID> -a pi:<configured model>
```

## Surfaces

- `lf task run`, `lf ship`, `lf -b <skill>` in a Task checkout, batch resume.
- `AgentProcessRequest` (`session_record.rs:905`) loses `write_scope` and
  `execution_boundary`; stored captures that carry them must still load for
  `lf replay`.
- Task status: `task_configuration_refusal` shrinks to agent-selection errors
  and loses its unused `skill` parameter.
- Docs: `docs/config.md:965-987` (also fixes `permission` → `permissions`),
  `docs/security.md`, `harness/README.md` capabilities row.

## Absent and error states

- Harness not installed, in a list: skipped, next candidate runs.
- Harness not installed, pinned or last: one failed attempt, install hint as the
  Task's reason.
- Step fails before any provider starts: no second attempt; `lf task run` again
  is the retry.
- OpenCode asks a question with nobody there: the turn ends with a typed failure
  (or the question is declined so the turn continues, if 2.2.1's API allows);
  it never waits for the timeout.

## Exclusions

- Usage-limit and transient-failure classification for OpenCode and Pi
  (`classifies_failures: false`, `agent/mod.rs:1062`): a failed OpenCode or Pi
  step gets no fallback and no wait. Needs real failure payloads; OpenCode's are
  LOO-518.
- Native output schemas and turn limits; Graph output validation already covers
  route steps on both.
- Any write denial. Behavior on Claude, Codex and Pi is unchanged.

## What goes

- `AgentWriteScope`, `AgentExecutionBoundary`, `checkout_execution_boundary`,
  `Capabilities::supports_execution_boundary`.
- Provider-only worktree branches: `agent/mod.rs:611-616`, `735-752`,
  `provider_writable_roots`'s boundary test, `opencode.rs:1272-1276`,
  `pi.rs:142`, `codex.rs:673`'s scope test.
- Tests: `managed_task_scope_carries_exact_delivery_capabilities`,
  `execution_boundary_resolves_linked_git_and_control_roots`, the matrix's Pi
  special case and its `external_directory` assertion.
- Survives: Claude and Codex worker argv and params, apart from `--add-dir` of
  the main repository now appearing on worktree steps as it does on every other
  headless run.

## Slices

1. **Admit OpenCode and see what else breaks.** Delete the capability and the
   boundary types, set `skip_permissions` for unattended steps, add the
   four-harness worker test. Then run the real `implement` step on
   `opencode/gemini-4-carbon` and a Pi model. This is the slice that can prove
   the plan wrong: it shows whether the plugin's turn reports, turn completion,
   usage and Graph advancement hold for a worker, which no test has exercised.
2. **Launch refusals fail once.** Filter the agent order by installed CLI; stop
   the `task run` loop when the failed step started no provider.
3. **Headless OpenCode questions end the turn.**
4. **Cancel and resume, on both.** Interrupt a running worker and resume a
   stopped Task Session per harness; OpenCode's interrupt has no test today.
5. **Idle work sessions hear steers.** Wake is Claude and Codex only
   (`rust/loopflow/src/server/README.md`, "Waking an idle conversation").
   OpenCode has a private server to post to; Pi's TUI has only its extension.
   The likeliest piece to become its own Task.

## Done when

`lf task run` on a Task pinned to OpenCode Gemini 4 Carbon, and on one pinned to
a Pi model, settles `implement` and moves to the next step; pinning a harness
that is not installed prints the install hint after one attempt.
