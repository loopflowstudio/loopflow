# Simplification Opportunities: `lf …` to a running harness

Read on LOO-823's branch at `8e1c71d6d5` (bare-direct built there); file:line
references are to that revision. Requested by Jack Heart, 2026-11-11: "this needs
to be simplifeid and unified. I wonder if there is a larger reduction to be done to
make sure that teh codeflow and datastructures and functions map today's paradigms
rather than being retrofitted onto yesterdays", then "i just think you should
/reduce the code flow from `lf ...` to launching a harness".

## Product intent

`lf` launches an agent with the right context. A scope's ongoing conversation
(Task, Wave, repository: `PrimaryScope`) is managed: the Machine server holds it,
it survives terminal close and detach, and it gets every hook and steers.
Everything else is bare, and behaves like the harness alone ("for a bare session, it
jsut does whatever claude or codex would do"). Headless runs and Graph steps live
as long as their caller.

## Opportunity 1: The owner re-runs the CLI instead of holding a Session

**Misalignment**: An owner exists to hold a managed conversation's provider. But
`transport::delegate` hands the owner the whole argv, and `transport::serve`
(transport.rs:863) runs `lf <same args>` again inside its PTY. The inner `lf`
dispatches a second time, sees `transport::current()`, compiles the prompt and spawns
the harness. It then hands its exit back over the socket (`hand_exit_record`,
transport.rs:2901). The process tree is viewer `lf` → owner → inner `lf` → provider.

**Symptom**: Three entry points, `delegate_current`, `delegate_session` and
`delegate_prepared_session`. `current()` checks are scattered across launch code:
`hand_to`, `native_args`, Codex hooks, `letting_go`, `open_without_replacing`.
There is an exit-record handoff protocol, and `dispatch` has an `execution`
allow-list of command shapes (bin/lf.rs:2331).

**Realignment**: Resolve the Session and prompt once, in the calling `lf`. A managed
Session goes to its holder as a prepared launch (Session id plus provider argv),
and the holder spawns the provider directly. `delegate_prepared_session` already
works this way for resume: "Open the resulting Session … without repeating the
command that prepared it".

**Cascade**: One delegation call. No second dispatch. `current()` reduces to "am I
the holder". The exit-record handoff goes. LOO-824's holder receives a Session, not
a command line.

## Opportunity 2: One `Holder`, decided once, instead of six "managed" tests

**Misalignment**: The model has one question: is this Session a scope's ongoing
conversation? The code asks six others:
- whether stdin is a terminal (`runs_in_terminal`, transport.rs:196);
- the command's shape (`bare_conversation`, bin/lf.rs:425);
- the stored scope, on resume only (`opens_bare`, human_session.rs:1142);
- which flags were passed (`Cli::names_work`, lf/mod.rs:468; `resumes_work`,
  session.rs:702), carried as `AgentConfig.managed`/`SessionCommand.managed`;
- whether an owner exists (`native_args`, harness/context.rs:130; Codex hooks,
  codex.rs:734);
- whether the run is interactive (`steer_environment`, agent_process.rs:146).

**Symptom**: These checks disagree. Each case below was read in code, not run:
- **`lf implement --task LOO-12` at a terminal:** it runs direct with only the
  context hooks, yet `managed=true` requires the server and reserves a steer
  capability.
- **`lf resume <Task's primary id>` without `--task`:** an owner holds it with every
  hook, but the server is never checked.
- **`lf implement` from an agent's Bash tool:** the conversation is bare, but it gets
  an owner, every hook and a steer capability.
- **Bare OpenCode:** it gets turn hooks (`native || managed`, agent/session.rs:226).

**Realignment**: One enum in `session.rs`, carried on `AgentConfig` and
`SessionCommand`:

```rust
pub enum Holder {
    Terminal,             // bare: foreground leader, two context hooks, no server
    Server(PrimaryScope), // managed: holder PTY, every hook, server required, steers
    Run,                  // headless runs and Graph steps: the caller's lifetime
}
```

It is decided once, where the Session id is known:
- `batch` or a Graph step gives `Run`;
- a new conversation gives `Terminal` (a new Session is never a scope's);
- a resume gives `Server(scope)` when `primary_scope(id)` is `Some`, else `Terminal`.

Flags only select the Session.

**Cascade**: These are deleted:
- `bare_conversation` and its Skill-vs-Graph resolution;
- `opens_bare`, and `runs_in_terminal` as a decider;
- `names_work`, `resumes_work`, `work_flag`'s managed role, and
  `journal::task_run_step` with its static;
- both `managed` fields, and the `session.interactive` steer branch.

The server check, steer capability, `hand_to` and delegation each read `Holder`. The
"managed rule" in `docs/dev/server-first.md` gets rewritten; it still names
`names_work`.

## Opportunity 3: Harnesses each re-derive launch policy

**Misalignment**: Hook set, server need and observers follow from the launch kind,
but each harness computes them on its own:
- Claude, Pi and OpenCode use `config.managed` or `native || managed`;
- Codex uses `(native && current().is_some()) || managed`;
- Codex's native path always starts an app-server, a `--remote` client and a
  setsid `__codex-observe`, whatever the launch kind (lf/commands/util.rs:499, :743).

**Symptom**: Bare Codex is not what plain `codex` would do, and bare OpenCode gets
turn hooks.

**Realignment**: `ContextDelivery::hook_settings(holder)` is the one place a hook set
is chosen. Harness modules only render that set into their own format: Claude
settings JSON, Codex session flags, Pi/OpenCode specs. For `Terminal`, Codex runs as
plain `codex` and its rollout is read at exit. The app-server and observer exist only
for `Server`.

**Cascade**: The per-harness `managed` and `native` parameters (`claude_args`,
`pi_args`, `codex_hooks`, `add_opencode_context`) collapse to `holder`. The bare
Codex observer and its first-turn submission go, once bare Codex receives its first
turn as a launch argument.

## Aligned areas

- `PrimaryScope` already is the managed concept. Keep it as the only input.
- `os_process::foreground::hand_to` is provider-independent. Keep it, applied to
  every `Terminal` launch.
- `ContextDelivery::hook_settings` is already the single source of hook JSON. Keep it,
  keyed on `Holder`.
- `delegate_prepared_session` is the model for Opportunity 1: hand over a prepared
  Session, not a command.

## Open choices

- Headless `--task` runs: every hook (today's `config.managed`) or only the two
  context hooks (server-first's "Pieces" table)?
- A `Terminal` launch whose stdin is not a tty (an agent's Bash tool): run attached
  to the caller's pipes like `claude -p`, or keep borrowing an owner PTY?

## Where each piece goes

| Piece | Task | Size |
|---|---|---|
| Opportunity 2 for bare: delete `bare_conversation`/`opens_bare`/`runs_in_terminal`; context hooks only for `Terminal`; drop the bare steer and exit-record handoff | LOO-820 | M |
| `Holder` from `PrimaryScope`; delete `names_work`/`resumes_work`/`task_run_step`/`managed`; server check on `Server` | LOO-820 ("fold it in here"); `managed` fields done | S–M |
| Opportunity 1: holder spawns the provider from a prepared Session | LOO-824 (its holder is the receiver) | part of LOO-824 |
| Opportunity 3 for Codex: bare runs plain `codex`, rollout read at exit | LOO-820 ("fold it in here") | M |
| Observers only for `Server` | one-line follow-up after LOO-826 ships | XS |
