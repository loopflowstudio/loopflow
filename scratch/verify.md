# LOO-968 verify, 2026-11-19

Outcome: **Verified.** PR #1976 merged as `21d541a874`; its tree equals this checkout's `14c8d802db`.
Checked with `target/debug/lf` (`0.16.0+14c8d802d`) in an `lf-world build task-review` world: own `LF_HOME`,
stand-in `claude` and `gh`, graphs `qaship` (implement, interactive ship-review, cmd land, interactive verify)
and `qaland` (implement, cmd land). World stopped after: launchd job booted out, no process left.

| Command | Printed |
| --- | --- |
| `lf task create … --json`, `lf task auto off <id>` | `8c5441: auto off`; status `Not checked out … · auto: off by Demo` |
| `lf task run <id> qaship` | log ends `Stopped at ship-review; the Task waits there for its conversation.`; status `active · auto: off by Demo` |
| `lf task auto on <id>` | `standing in at ship-review`, `merges: this run may merge the PR; \`lf task auto off 8c5441\` stops it`; status `moving · auto: on by Demo`, `Graph qaship: running at land` |
| `lf ship <id>` / `lf task hold <id>` | `skill or graph not found: ship` / Clap usage error |
| `lf -b task run <id> qaland` | ran to land; `--json` `auto` is null |
| `lf task auto on <id> --graph qaland`, then `lf task auto off <id> --reason …` mid-implement | `Waiting at land: auto is off (Demo). \`lf task auto on 43482f\` lands it.` |
| `lf task complete`, `lf task reopen` | `auto` null |
| `lf config set auto on`, bare `lf task run <new> qaland` | `merges:` line; `moving · auto: on by personal config` |
| `lf task auto off <new>` then run under that config | no `merges:` line; `auto: off by Demo` |
| `lf config set auto off`, `lf config set graph auto` | both refused |

Not checked here: a real merge (the world's `gh` answers only reads; `land` fails "no active PR"), the Desktop
toggle, iOS, Machine moves. `lf config set auto off` reads `Error: invalid graph: auto: unknown variant \`off\``:
the shared error prefix says "graph" for a config value.

CI for `21d541a874`: merge queue `CI` green, push `CI` green; push `macOS on main` and `Packages (main)` still running at the time of writing.
