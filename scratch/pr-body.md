Planning skills now follow affected behavior through relevant docs, implementation, callers and tests, and keep the few referenced facts that explain the active plan. Compress and realign correct stale findings and remove facts that no longer help.

Bare `claude` can start on a saved `claude.model` without changing the selected harness or overriding an explicit model such as `claude:fable`. Account routing, native launches and recorded requests use the same model. Resume retains its recorded model unless a new choice is supplied; bare Claude keeps capacity fallthrough.

Config file reads warn and skip unknown preferences, invalid values and malformed YAML entries. Valid neighboring settings remain usable, comments are silent, and reads leave files untouched. Filtering also applies before forwarding personal preferences. Live subscriptions retain their valid fallback, and an invalid graph default falls back to `code`. Explicit setters and invocation overrides remain strict. Normalize the released `workflow` spelling before filtering so a Wave's graph choice survives.

Validation:
- 94 configuration library tests, the CLI bad-config regression, `two_people`, live subscription and Task-default tests, and all eight Wave setup tests passed locally.
- 21 launch/resume tests and focused routing, prompt-golden and documented-command checks passed earlier in this change.
- Formatting, Clippy with warnings denied, architecture coverage and test inventory checks passed.
- A real bare-Claude probe completed successfully; its Session recorded `claude` / `opus`.

CI exposed three old assertions expecting invalid config to fail; they now test the fallback behavior and are included in config gate guidance. An unchanged quiet-deadline test also failed once in CI and passed locally without edits. The parallel configuration run's Git-child counter captured neighboring fixtures; its serial run passed. CI runs tests in isolated processes.

After merge, install a main build containing this change before adding personal `claude.model: opus`; older binaries reject that key. Verify the installed CLI's effective preferences, Graph preview and a bare-Claude launch. Fable overrides remain commented while capacity is exhausted; model-specific fallback is separate work in LOO-834.
