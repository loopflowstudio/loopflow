---
layout: default
title: Native authentication
---

# Native authentication

Sign in with each provider's own CLI, then run work through Loopflow:

```bash
claude                          # complete Claude Code sign-in
lf -a claude implement

codex login                     # sign in to Codex
lf -a codex implement
```

Each launch uses the native client's authentication on the Machine where it
runs. Claude Code and Codex own login, logout, token refresh, and subscription
limits (their API-key configurations work too). Loopflow never polls quotas,
switches logins behind your back, or proxies your API traffic.

## Homes and conversation history

Claude uses `~/.claude` (or `CLAUDE_CONFIG_DIR`); Codex uses `~/.codex` (or
`CODEX_HOME`). Changing your native login applies to subsequent launches.
`lf resume <session-id>` keeps the native conversation ID without copying
credentials, and reports missing history instead of starting a blank chat.

## When a provider refuses work

```bash
lf set --global agents claude,codex
lf -b implement
```

When headless work hits a classified usage-limit or model-unavailable error,
Loopflow moves to the next entry in your `agents` order (`codex: provider usage
limit; continuing on claude`). Switching harnesses seeds the new conversation
with a plain log of the work so far. See
[Keep going when a provider runs out](config.md#keep-going-when-a-provider-runs-out).

| Launch | After a classified refusal |
|---|---|
| Graph step, with or without a Task | Follows configured model/harness fallbacks |
| Other headless work (`lf -b <skill>`, commit/PR copy, sync conflicts) | Follows fallbacks and stops when the order runs out |
| Interactive, with or without a Task | Keeps the thread and refusal visible; wait for your retry or `lf -a <harness> resume` |
| Explicit agent (`-a` or `agent:`) | Keeps the harness (bare Claude still tries its other models; `claude:opus` pins the model) |
| Graph step's order exhausted on a usage limit | Wait 5, 15, 30, then 60 minutes, retrying from the first entry after each; then stop with the error |
| Revoked login | Skips that provider's remaining models; tries the next provider or stops with sign-in instructions |
| Configured harness missing or unsigned-in on this Machine | Falls back to the first usable configured fallback, then a usable installed native harness; explicit `-a` and established threads keep their harness |
| No usable harness or login on this Machine | Reports which executables or logins are missing and stops recovery until inputs change (no backoff retry loop) |

Before selecting a new conversation or worker, Loopflow checks the executable
for each candidate. For Claude and Codex it also checks the native client's
login status or the presence of a forwarded login token. Other clients handle
their model-specific authentication when they start.

Each retry after a wait restarts at the first entry of `agents` using whatever
native login is active on the Machine, so renewing usage or signing in picks up
automatically. `lf task status <ID>` shows the wait and next retry time.

## Remote Machines and rentals

```bash
ssh mini                                      # sign in natively on a Machine you own
lf --machine mini -a codex implement
```

Machines you own keep their own native logins. On-demand rentals start blank,
so opt in to forwarding explicitly named tokens or auth files:

```bash
claude setup-token                                       # once, on this Machine
doppler secrets set CLAUDE_CODE_OAUTH_TOKEN > /dev/null  # save the token
lf set --global forward.env '[CLAUDE_CODE_OAUTH_TOKEN]'
lf set --global forward.files '[~/.codex/auth.json]'
```

| Setting | Behavior on remote launch |
|---|---|
| `forward.env` | Reads each named variable from this Machine's environment, else Doppler |
| `forward.files` | Copies each home-relative file (`0600` in a `0700` dir) when the destination has none |

- Values and files travel over SSH stdin, never in command arguments, logs, or
  the store.
- A destination that already has a named file keeps its own on ordinary
  launches (and logs if it differs). Missing local variables or files stop the
  launch before renting.
- To push rotated `forward.files` onto a live Machine, run `lf machine forward`:

  ```bash
  lf machine forward modal-1                      # replace configured forward.files atomically on modal-1
  lf machine forward modal-1 --task LOO-570       # refresh files, then restart LOO-570's primary Session
  lf machine forward modal-1 --session session_…  # refresh files, then restart that Session only
  ```

  Each file is replaced atomically via a private temporary file over SSH stdin.
  Passing `--task` or `--session` restarts only that Session's provider on its
  owning Machine after delivery completes, preserving the Session and its
  continuation; unrelated Sessions keep running, and a delivery failure
  restarts nothing.
- **Claude:** forward `CLAUDE_CODE_OAUTH_TOKEN` from `claude setup-token`.
  Loopflow strips ambient `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, and
  `CODEX_API_KEY` from Claude/Codex launches so shell keys don't silently move
  subscription runs onto API billing.
- **Codex:** forward `~/.codex/auth.json` on Plus/Pro (treat it like a
  password; note that concurrent refreshes across copies can sign each other
  out; prefer a dedicated login, `codex login --device-auth`, or
  `CODEX_ACCESS_TOKEN` on Business/Enterprise).

## Service authentication

```bash
lf account connect linear --chrome-profile Work
lf account connect github
lf account connect opencode
```

`lf account` manages service logins for GitHub, Linear, Asana, Notion, Doppler,
and OpenCode, separate from native Claude/Codex logins.
