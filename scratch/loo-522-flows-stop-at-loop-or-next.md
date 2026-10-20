# LOO-522: Flow steps lose the transport handoff at the installed entry gate

Status: implemented in the working tree (2026-10-19), uncommitted; Docker
installation proof and installed acceptance remain. Jack Heart filed the Task on
2026-10-19; its stated outcome and constraints are unchanged.

## Problem

`lf task run <issue> pursue` stops at `loop-or-next` with
`loop-or-next captured no Session input` after that step exits 0. Operators
publish by hand and move the Task's Workflow back.

## What the store shows (2026-10-19, read-only against `~/.lf/loopflow.db`)

The brief's hypothesis was that the step attached to the Task's live conversation.
The store refutes it: each step captured its own fresh Session, but on a
transport-served grandchild instead of on the step Process.

- Flow `2ebf4e2d…`, step `11e5d017` (`lf skill loop-or-next`) → child
  `b314cb50` (`lf transport serve`) → `ed5d2489` (the same command, served).
  The capture (`session_4e91…`) sits on `ed5d2489`.
- Second run: `lf task run` `35219093` → `lf flow` `b4ed034f` → `transport serve`
  `89424c85` → Flow `0630bd29`. Every agent step under `0630bd29` again delegated
  to its own `transport serve`. `loop-or-next` `aa3bb1a9` captured on `8663afc7`.
- Installed lf 0.14.7 predates #1600 (LOO-517, `a572d1788c`), which makes
  `process_input` follow delegation. Running #1600's query on both failing steps
  returns their captures, and `input_final_answer` returns `{"decision":"advance"}`
  for both. Under v0.14.8 both Flows would have advanced.
- Across all Flow steps, re-delegation went from 0% (Oct 11–17) to 100% from
  Oct 18 onward, which is when lf-08d39 was installed with #1581's transport handoff.

### Root cause

#1581 (`28dc141671`) gives a Flow and its steps one terminal owner.
`transport::configure_child` passes `LF_TRANSPORT_OWNER`/`_PARENT`/`_CONTROL` to
each step, and `delegate_current()` skips delegation when a handoff exists. But:

1. `bin/lf.rs` `main()` calls `transport::initialize()` first. It consumes the
   handoff into `HANDOFF` and **removes the env vars**.
2. `run()` then calls `installation::dispatch_entry_gate`, which `exec`s the
   installed artifact with the now-stripped environment
   (`installation.rs:862`; `dispatch_default_cli` at `:791` behaves the same way).
3. In the installed path, `resolve_lf_binary()` returns the gate
   (`~/.lf-machine/install/gates/1/lf`), so every Flow step, and the Flow under
   `lf task run`, goes through that exec. The artifact sees no handoff and
   delegates again.

Tests never see this. Fixtures select a custom home with `LF_BIN`, so
`resolve_lf_binary()` skips the gate and the handoff survives.

### Consequences of the lost handoff

- Answers were read from the wrong Process. #1600 now follows delegation, so this
  is fixed on main but not installed.
- `lf task run` retry is dead. `flow.rs:158` checks
  `store.flow_process(child)` on its direct `lf flow` child, but the FlowProcess
  row is on the served grandchild. So `failed` is false and a failed Flow is never
  retried. This also explains why `lf flow show` on the outer ids found no Flow.
- Each step runs its own owner, printing `attached; /status…` per step, instead of
  sharing the Flow's single owner as `configure_child` documents. That means an
  extra `transport serve` Process per step.

## Demo

From an lf-launched shell, with the fixed build installed:
`lf task run LOO-xxx pursue` reaches `pr-publish` without manual steps.
`lf flow show <flow> --processes` lists steps whose Process tree has no
`transport serve` child. One attach line appears for the Flow, not one per step.
Before installation, the disposable-OS installation test shows the same tree shape.

## Approach (chosen)

Preserve the handoff across same-PID re-exec. An exec keeps the PID and the
parent PID, so the handoff's direct-parent check still holds for the exec'd
artifact. Only the env removal breaks it.

- `transport.rs`: `initialize()` keeps the consumed owner, key **and parent PID**
  in `HANDOFF`. Add `pub(crate) fn carry_across_exec(command: &mut Command)`,
  which restores the three variables with the original parent PID when a handoff
  is held. It sits beside `configure_child`; that function stays for real children
  (parent = this PID).
- `installation.rs`: both exec sites (`dispatch_default_cli`,
  `dispatch_entry_gate`) build their command through one private
  `fn reexec(path: &Path) -> Command`. It copies `args_os().skip(1)` and calls
  `carry_across_exec`. That gives one implementation of "become another lf binary".
- `flow.rs` `answer()`: when nothing was captured, name the step Process:
  `"{label} step Process {id} captured no Session input"`. When no step Process
  registered, say `"{label} registered no step Process"`. When a capture exists,
  its Session is already in the record, so the message has nothing more to name.

Once the handoff survives, steps run inside the Flow's owner and capture on their
own Process, and `lf task run`'s child `lf flow` records the FlowProcess on itself,
so the retry check works as written. #1600's delegation-following lookup stays:
a directly invoked `lf skill`/`lf flow` from a terminal still delegates by design.

### Alternatives considered

- **Move `initialize()` after the exec points.** It is a smaller diff, but
  `dispatch_default_cli` runs after Clap parse and in the remote/ssh branch.
  Anything spawned before the moved call (ssh, for instance) would inherit the
  control key. Rejected: it trades one ordering hazard for another.
- **Stop removing the env vars.** The parent-PID check alone keeps children from
  adopting the handoff, but the control key would leak into provider and tool
  environments. Rejected because #1581 removes the vars deliberately.
- **Make `flow_process(selector)` / the retry check follow delegation.** That
  treats the symptom. Once steps stop delegating, the direct child holds the row.
  Excluded; reconsider only if a legitimate delegated Flow needs selection by its
  outer id.
- **Retry or tolerate missing answers.** Forbidden by the Task's constraints.

## Scope

In: handoff across gate/default-CLI exec; the step-naming error; tests below.
Excluded: installing v0.14.8 or a later build (Release owns that; until then,
installed Flows still fail at routing steps); `lf flow show` by outer delegating
id; Desktop attach presentation.

## Delete — do not maintain

Nothing to delete. #1600's delegation walk in `process_input` remains correct
for top-level delegation. The fix removes per-step `transport serve` Processes
at runtime, not code.

## Forbidden outcomes

- A second answer channel, a stored answer copy, or a retry on missing input.
- Leaving `LF_TRANSPORT_CONTROL` in the environment past `initialize()` for
  anything other than a same-PID exec or a configured direct child.
- A separate exec path for gate and default CLI.

## Acceptance (gate runs once, headless)

1. Unit tests in `transport.rs` (done): a direct child's `Handoff::take` strips
   the env and `pass` with the original parent restores owner, key and parent PID;
   a non-child adopts nothing and `carry_across_exec` adds no env. They exercise
   `Handoff` directly because the lib test binary never sets the global `HANDOFF`.
   Env mutation holds `journal::test_env_lock` (wraps `loopflow_test_support::env_lock`).
2. Extend the disposable-OS installation test
   (`running_provider_uses_updated_install_for_commands_hooks_and_flow`, run by
   `scripts/test_task_installation.py`): the entry-proof Flow's step is an
   execution command launched through the gate (for example `cmd: flow inner --batch`
   with `inner` = `- cmd: sync --plan`). Assert that the step Process has no
   `transport serve` child and the Flow succeeds. It fails before the fix
   (the step re-delegates). Needs Docker; defer to CI if unavailable locally.
3. Existing `custom_router_returns_a_captured_path_without_an_in_turn_command`
   and #1600's `step_input_is_read_through_transport_delegation` still pass.
4. `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.

## Done when

- A Flow launched through the installed gate runs its steps inside the Flow's
  transport owner (no per-step `transport serve`), proven by acceptance 2.
- Routing steps advance through the installed path once the build is installed.
- A genuinely missing capture names the step Process.

## Remaining

- Acceptance 2 (Docker installation proof, written: `flow inner --batch` nesting,
  two successful steps, no `transport serve` child) has not run; gate/CI owns it.
- Installed acceptance (`lf task run … pursue` reaching `pr-publish` on a real
  Task) waits on a release containing this change and #1600.
- Commit and delivery.

Check: `cargo test --lib transport::tests` + router test pass, clippy clean; Docker proof deferred to gate/CI.
