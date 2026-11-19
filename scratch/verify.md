# LOO-959 verify, 2026-11-19

Outcome: **Follow-up.** Every outcome claim holds on main; one leftover outside them is filed as 81de28.

Main is `f5ff6a6c79` (PR #1972, merged 2026-11-19T10:57:32Z). `git diff --quiet 495a3aa021 origin/main` exits 0,
so this checkout's build is main's. Run on a private Machine: `lf-world build one-task /tmp/v959/w`, two more
Tasks, the built `lf server run` detached, world `claude` stand-in. Recipe: `.lf/qa/task-conversations.md`.

## CI for f5ff6a6c79

- merge_group CI 44508891454: success.
- push CI 44510419625: success (rust-build, rust-lint, e2e-smoke, task-installation, merge-proof; rust-test, macos, migration-check skipped on push).
- push "macOS on main" 44510419700 and "Packages (main)" 44510625646: in progress when read. Not passed.

## Claims

| Claim | Command | Printed |
|---|---|---|
| Killed conversation runs again, nobody resuming | `kill -9 <claude pid>`, poll `lf task status 78f3db` | `Conversation: died 02:59 (exit 137) · starting again`, 4 s later `Conversation: running (started again 02:59, attempt 1)`; one provider |
| From its saved thread | transcript written for 60ca12, `kill -9` | new provider argv `--resume 92452bae-…` (same id) |
| Quit stays quit | `quit` through `lf resume --task 16e5a5` in a PTY | `Conversation: quit 02:59; \`lf resume --task 16e5a5\` opens it`; same 65 s later and after the reboot; 0 providers |
| After a Machine restart, what ran comes back | SIGKILL server, holder, both providers; new server | within 1 s both `running (started again 03:01, attempt 2)`, each `--resume <its own thread>`, one provider each; quit one still quit |
| Server restart alone | SIGTERM server, start again | both still `attempt 2`, 2 providers |
| Died run is reported with its step, continued once | `kill -9 -<lf task run>` and its step | `Graph qarun: stopped at implement` / `Run died at implement; the server's next pass carries it again`; 54 s later one `lf task run 78f3db` (parent: server) at `implement` |
| A continued run never approves Auto-Ship | release the step | `Graph qarun: at human-review`, `finished; nothing further is launched`, 0 carriers, no Auto-Ship line |
| doctor PTY line | `lf machine doctor` | `ok ptys 111 of 511 PTYs in use (kern.tty.ptmx_max), 400 left; each Task conversation holds one` |

With no server running, the two killed conversations read `closed; \`lf resume --task X\` opens it`
(PR open decision 3): what stopped is not named, the command is.

## Follow-up 81de28

The killed Graph process row kept `ended_at` NULL: `lf task status` listed it `Current` beside its
`Completed` replacement for over 70 s, until a server restart set it `Stopped`.

## Not checked here

A real reboot under launchd, real Claude's exit code on SIGTERM at logout, Desktop's line in a live window.
These need a person or a release; none passed.
