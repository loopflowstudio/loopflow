# Troubleshooting

When something stops, check three things in order: what survived, what went
wrong, and what to run next.

## 1. What survived

Crashes, reboots, and closed terminals lose very little:

| Survives | Where it lives | Resume with |
|---|---|---|
| Conversations (interactive & headless) | `~/.lf/loopflow.db`, `~/.lf/history.db`, and native provider transcripts | `lf resume --task <ID>` or `lf resume <SESSION>` |
| Task worktree, branch, PR, and thread | Beside your repo and on GitHub/tracker | `lf task run <ID>` |
| Task work sessions and runs | Auto-restarted by the server unless you quit or interrupted them | `lf task status <ID>` |
| Stopped Graph runs | Recorded as history with per-step exit codes | `lf task runs <ID>` · `lf graph show <RUN_ID>` |
| Uncommitted files | In the worktree (`lf wt prune` saves unpushed work to `refs/lf/pruned/…` first) | `lf wt list` |

## 2. Diagnose what happened

```bash
lf doctor                                    # installation, store, lock contention, PTYs, stray processes
lf task status <ID>                          # Graph step, conversation state, and reason
lf graph show <RUN_ID>                       # per-step status and exit codes
lf session history show <SESSION_ID> --final # agent's final output
lf session list                              # live conversations
lf graph list --runs --state current         # live Graph runs
```

## 3. Common fixes

| Symptom | Command |
|---|---|
| Task is waiting for your input | `lf resume --task <ID>` |
| Run is heading the wrong way | `lf steer <ID> "address the latest feedback"` |
| Need to stop it right now | `lf task interrupt <ID>` |
| Stopped at a failed step, ready to retry | `lf task run <ID>` |
| Need to redo an earlier step | `lf task run <ID> --from <step>` |

## Desktop cannot connect

```bash
lf server status   # readiness, socket, build, and connection error
lf doctor          # installation and store health
```

- **Opening… Input paused / Reconnecting…** — agents run in a separate
  `lf holder run` process, so they keep working while the server restarts. Keep
  the pane open; click **Refresh session** or **Retry connection** if automatic
  retries stop (neither restarts the agent or replays input).
- **Disabled background access** — re-enable Loopflow in macOS System Settings
  (or the Linux user service).
- **Build mismatch** — run `lf install` (do not start a second `lf server run`
  by hand).

## "Loopflow server is not running"

```bash
lf install server
lf server status
```

Task/Wave launches and Desktop shells require the local server (plain `lf`,
`lf <skill>`, and `lf graph` do not). For a remote Machine, run
`lf --machine <machine> install server`.

## Rate limits and usage refusals

Headless runs retry transient capacity errors 4 times (up to 30 s backoff) in
the same conversation. Classified usage-limit or model-unavailable errors move
through your `agents` list; once exhausted, Graph steps wait 5, 15, 30, then
60 minutes before stopping.

| Reason prefix in `lf task status` | Meaning | Graph step behavior |
|---|---|---|
| `provider_rate_limited:` | Account usage limit reached | Moves to next `agents` entry; waits if all spent |
| `provider_model_unavailable:` | Model unsupported or misspelled | Moves to next `agents` entry |
| `provider_credential_invalidated:` | Login revoked or unrefreshable | Skips provider's models; tries next provider |
| `provider_capacity:` | Provider overloaded | Retries same conversation |

```bash
lf set --global agents claude,codex
lf config settings --skill gate --explain
```

Sign in again natively on the execution Machine if credentials expired, then
continue with `lf task run <ID>`. See [Native authentication](subscriptions.md).

## Agent not found

```bash
npm install -g @anthropic-ai/claude-code && claude
```

With multiple harnesses installed and no default set, run
`lf set --global default_agent claude` (or `lf set --global agents claude,codex`).
For OpenCode, install OpenCode 2+ (`npm install -g @opencode/cli`).

## A steer arrived late or a turn is missing from history

```bash
lf server status
grep 'recording gap' ~/.lf/logs/lf.log | tail
```

If the server is slow or restarting, turn hooks time out after 2 seconds so
your conversation never stalls, logging a `recording gap` and a `store-gap`
entry in history. Steers read directly from the store and retry after 15
seconds if undelivered. If `lf server status` reports a mismatched build, run
`lf install`; to refresh hooks in an older running Session, quit its agent and
run `lf resume`.

## Setup failed or unconfirmed Task creation

- **Setup failed in a new Task's conversation** (`Setup stopped: checkout failed`):
  fix the reported Git/worktree error and run `lf resume --task <ID>` to retry
  setup.
- **Lost reply on creation / `outcome_unknown`:** inspect before retrying so
  you don't duplicate work:

```bash
lf server receipt REQUEST_ID --json
lf task status <ID> --json
lf task runs <ID>
```

## Context too large

```bash
lf --context qa --docs src/small/            # preview input without launching
lf session usage --context --task <ID>       # breakdown by source against budgets
```

Narrow `--docs` or `context.docs` in `.lf/config.yaml`, or trim stale notes in
`MEMORY.md` / `scratch/`.

## Disk filling up, stale worktrees, and branches

```bash
lf wt prune --dry-run                        # preview finished worktree cleanup
lf wt prune                                  # remove finished worktrees and branches
lf wt prune --branches --dry-run             # preview merged branch cleanup
lf machine prune                             # prune expired history (>90 days) and compact DBs
```

`lf wt prune` never removes unfinished Tasks, active processes, or open PRs,
and saves unpushed commits under `refs/lf/pruned/<branch>/<commit>` first.
`scripts/disk-cleanup.sh` also reclaims idle build caches (`target`,
`node_modules`, `.venv`, `DerivedData`) not held by any running process.

## Processes (`lf holder run`, strays, and `ps`)

```bash
ps -axo pid,etime,command | grep -E '[ /]lf( |$)'
lf machine doctor --json                     # lists stray agent processes by PID
```

Expect one `lf server run`, one `lf holder run` per build with open work
sessions (each exits a minute after its last conversation ends), and one `lf`
per active Graph or headless run. Stop leftovers individually by PID — never
`pkill lf`, which kills live work in other checkouts.

## A Wave isn't moving

Wave passes are finite (`lf --wave <wave> wave-operate`). Nothing runs between
passes unless scheduled in your own crontab or macOS LaunchAgent.

Next: [`lf` reference](lf-reference.md) for every command and flag.
