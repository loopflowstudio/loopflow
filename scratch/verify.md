# LOO-1078 verify (2026-11-26)

Merged as 472bacbd64 (#2036); branch HEAD ce1f04ee4f has the same tree.

Post-merge push CI: `CI` run 45925453944 completed success. `macOS on main` run 45925453938 still in progress (deferred).

Ran headless with `target/debug/lf`, isolated HOME/LF_HOME, LF_* unset, in a throwaway repo:
- `lf task pause` on unstarted Task → `Error: Task has not started; nothing to pause`
- after `lf task run -a claude` (failed: no Claude login in isolated HOME): `lf task pause` → `19f004 paused` (0.13s)
- status: `Graph code: Paused in plan at kickoff. `lf task unpause --run` resumes it.`; owner kept
- `lf task list --state paused` → `19f004 · V · Paused · pause probe`; `--json` has `"working_group":"paused"`
- `lf task run` and `lf task auto on` while paused → `Error: Task is paused; run `lf task unpause 19f004` to lift the hold`
- `lf task unpause` → `19f004 unpaused`; status back to `Stopped ... lf task run starts it again`
- `cargo test -p loopflow --lib -- pause_stops_the_worker_and_keeps_its_task_conversation_open pause_during_interrupt_holds_the_interrupted_step` → 2 passed

The builtin `code` Graph on main has no verify step: #2049 ("Finish Tasks at merge instead of waiting at a verify stop") landed after this work and supersedes item 3 of the 2026-11-24 revision. The PR body's line about verify is out of date, but main's behavior is intentional.

Not checked here: Mac/iPhone workflow track visuals (macOS CI pending).
