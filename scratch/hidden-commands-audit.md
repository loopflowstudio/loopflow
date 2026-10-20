# Hidden `__*` entry points: audit

Jack asked for this audit on 2026-10-20 because hidden commands are a smell. Counts are from the
`processes` table in `~/.lf/loopflow.db` over the last 7 days, 598,520 rows in total. Nothing
below has been changed yet.

| Command | Invoked by | Runs / 7d | Proposed verdict |
|---|---|---|---|
| `__context-block --moment tool` | Claude/Codex PostToolUse and UserPromptSubmit hooks | 31,161 | Done on this branch: PostToolUse runs `lf` only when `steers/latest` is newer than the provider's seen marker, and prompt boundaries keep the full check. The 1,922 failures from deleted worktrees are tracked as LOO-543. |
| `__planning-check` | Stop hook, Pi `agent_settled`, OpenCode | 1,158 | Done on this branch: merged into `__context-block --moment stop`. Turn end acquires Linear changes, and the next boundary delivers them as steers. |
| `__session-title` | Global `~/.claude/settings.json` and `~/.codex/hooks.json` written by `lf install` | 3,572 | Deleted on this branch (Jack, 2026-10-20). It was lf's only write to global provider settings, used to title self-started conversations, and it caused the October prompt timeouts. Machines that ran `lf install` keep a stale `lf __session-title` entry, which must be removed by hand. |
| `--__cron-receipt` / `--__cron-lock-fd` | cron | 10 | Done on this branch: `LF_CRON_EXECUTION=<receipt>:<fd>`. Flow steps inherit it, and `lf repo release run` reads it. |
| `__pi-event` | Pi extension, blocking `execFileSync` on every event | 0 (two days old) | Write JSON lines over a pipe the owning process already holds. Skipped (Jack, 2026-10-20). |
| `__tool-lifeline` | Pi, in front of every bash call | 0 | Have the parent watch the process group (unverified). |
| `submit --create-pr` | — | — | Done on this branch: deleted. |
| `__telemetry-scorecard` | `telemetry-daily` Flow | 4 (41.8s each) | Fold into a public view or a skill. |
| `--__cwd` | `human_session.rs` | 84 | Use `current_dir`, with the binding keeping the cwd (unverified). |
| `task __repair` | `pr_landing.rs` | 6 | Use the public `ci-fix` skill launch with the reservation in an environment variable (unverified). |
| `--skill-input` / `--output-schema` | Flow runner | 829 / 142 | Move to environment variables. AGENTS.md says a step is a plain command. Skipped (Jack, 2026-10-20). |
| `__context-block` start/compact | SessionStart hook | 652 | Keep, merged into a single hook entry. |
| `__transport-serve` | `transport::start` | 256 | Keep. A detached owner must exec as its own process. |
| install `*-switch` / `promote` / `rollback` | Installer | 17 in 30 days | Keep. These cross versions on purpose. |
| `cron run --scheduled`, `--parent-pid`, `--request` | launchd, Desktop | — | Keep. |

History noise: hook rows are about 6% of the ledger. Desktop `task file` and `task diff` polling
is about 90% and is the bigger source.

Precedent: #1552 deleted `__provider-session`. It replaced the hook with identity from the
channel lf already owns.
