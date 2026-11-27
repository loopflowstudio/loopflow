---
layout: default
title: Native authentication
---

# Native authentication

Sign in with each provider's own CLI, then run work through Loopflow:

```bash
lf machine login claude         # runs Claude Code's own sign-in
lf -a claude implement

lf machine login codex          # runs Codex's own sign-in
lf -a codex implement
```

Signing in with `claude` or `codex login` yourself works the same.

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
| Revoked or missing login | Skips that provider's remaining models and tries the next provider, or stops with sign-in instructions |
| Configured harness missing or unsigned-in on this Machine | Falls back to the first usable configured fallback, then a usable installed native harness; explicit `-a` and established threads keep their harness |
| No usable harness or login on this Machine | Reports which executables or logins are missing and stops recovery until inputs change (no backoff retry loop) |

Before selecting a new conversation or worker, Loopflow checks each candidate's
executable. For Claude and Codex it also asks the native client for its login
status. Other clients handle model-specific authentication when they start.

Each retry after a wait restarts at the first entry of `agents` using whatever
native login is active on the Machine, so renewing usage or signing in picks up
automatically. `lf task status <ID>` shows the wait and next retry time.

## Sign in on any Machine

Claude reaches other Machines as a token you forward; Codex signs in on each
Machine.

```bash
claude setup-token                                     # here: prints a long-lived token
doppler secrets set CLAUDE_CODE_OAUTH_TOKEN > /dev/null  # paste it at the prompt
lf set --global forward.env '[CLAUDE_CODE_OAUTH_TOKEN]'
lf --machine mini -a claude implement

lf machine login codex mini
#   Approve Codex for mini in Chrome (Work) with code ABCD-1234
#   mini: Codex signed in (ChatGPT)
lf --machine mini -a codex implement
```

**Claude.** Every remote launch sets `CLAUDE_CODE_OAUTH_TOKEN` on the
destination from this Machine's environment, else Doppler, and Claude signs in
with it for that run ([per-run secrets](#per-run-secrets)). The destination
keeps no Claude login of its own, and nothing is written there. The token is a
`claude setup-token`: Claude mints it, and you renew it the same way when it
expires. `lf machine login claude` with no Machine runs `claude auth login`
here; naming another Machine refuses:

```
Claude reaches mini as a forwarded token: run `claude setup-token`, store it with `doppler secrets set CLAUDE_CODE_OAUTH_TOKEN`, and add CLAUDE_CODE_OAUTH_TOKEN to `forward.env`
```

**Codex.** `lf machine login codex [<machine>…]` runs `codex login
--device-auth` on each named Machine, or on this one when none is named, and
opens its approval page here; you approve with the code it prints, within 15
minutes. Each Machine ends up with its own login, written and refreshed by the
unmodified client, and no credential crosses the network. Device code login
must be enabled in ChatGPT security settings. Running `lf machine login codex`
again replaces that Machine's login with whichever account approves it.

```bash
lf machine logins mini
#   claude    forwarded    CLAUDE_CODE_OAUTH_TOKEN
#   codex     signed in    ChatGPT
#   github    signed in    jack-heart
#   linear    signed out
lf machine logout codex mini
```

`lf machine logins [<machine>] [--json]` asks the Machine's own clients
(`claude auth status`, `codex login status`) and shows `unknown` when one can't
answer. On another Machine, Claude reads `forwarded` when your `forward.env`
names `CLAUDE_CODE_OAUTH_TOKEN` and this Machine has its value (exported or in
Doppler); named with no value here, it shows the Machine's own answer with
`CLAUDE_CODE_OAUTH_TOKEN has no value here`. Each row also shows the provider's last
refusal there, login missing or usage limit, and when. `lf machine logout` runs
each provider's own logout.

Every launch, local or remote, asks the same clients first, with the launch's
environment, so a forwarded token counts as signed in. A Machine with no
login for a provider skips that provider, like a [revoked login](#when-a-provider-refuses-work),
and with none left stops before an agent starts:

```
mini has no Codex login; run `lf machine login codex mini`
mini has no Claude login; launch where CLAUDE_CODE_OAUTH_TOKEN has a value (`doppler secrets set CLAUDE_CODE_OAUTH_TOKEN`, or run under `doppler run`) and `forward.env` names it (`lf set --global forward.env`)
```

An unknown answer launches anyway. To sign Codex in again wherever it is
signed out, or refused a launch with no Codex launch since:

```bash
lf machine login codex --refused
```

### Choose who approves

```bash
lf set --global approvers.codex.chrome Work
lf set --global approvers.github.command 'my-approver'
```

| `approvers.<provider>` | What a sign-in does |
| --- | --- |
| unset | Opens the default browser |
| `chrome: <profile>` | Opens that Chrome profile, by name or directory |
| `command: <cmd>` | Runs the command with `LF_APPROVE_PROVIDER`, `LF_APPROVE_URL`, `LF_APPROVE_MACHINE` and, when the page asks for a device code (Codex, GitHub, OpenCode), `LF_APPROVE_CODE`; exit 0 means approved |

The setting covers every sign-in `lf machine login` opens a page for: Codex,
GitHub, Linear, Asana and OpenCode. Claude's own login here opens its
page itself. `--chrome-profile <profile>` overrides the setting for one run.
The browser picks the account; Loopflow keeps no list of accounts. A command
approver needs no terminal, so unattended sign-ins use one.

### Rentals

A rental works as you on GitHub, in the one repository it runs, for at most 8
hours. Your own `gh` token stays on your Machine.

```bash
lf ondemand add code --provider daytona --vcpus 4 --memory-gb 8 --max 2 \
  --max-usd-per-hour 0.40 --logins codex
lf task run LOO-570 --rent code
```

```text
Rentals need GitHub for loopflowstudio/loopflow.
Approve at https://github.com/login/device with code ABCD-1234
Renting 4 vCPU 8 GB on daytona at $0.40/h until 03:40
GitHub on code-1 until 07:40
```

Once per repository, install the [Loopflow GitHub App](https://github.com/apps/loopflow)
on it. A repository's first rental then asks for one approval, before anything
is rented; later rentals of that repository ask for nothing.

What the rental holds:

- **One repository.** The token clones, pushes and opens pull requests in the
  repository the rental runs and reads no other, so a private submodule or Git
  dependency kept in a second repository does not clone there.
- **Your name.** Commits and pull requests are yours, as they are from your
  own Machine.
- **No workflow edits.** GitHub refuses a push from a rental that adds or
  changes a file under `.github/workflows/`. Run that Task on a Machine you
  own.
- **8 hours at most.** Placement prints when GitHub stops on that rental. The
  token stops sooner, when the repository's last rental is given back.

```bash
lf ondemand login github          # approve ahead of time, from the repository's checkout
lf machine logins code-1          # github reads signed out once the token stops
lf machine login github code-1    # send a fresh token to a rental that outlasts it
```

`lf ondemand login github` is the approval by itself. Run it before a
placement that has no terminal, such as a scheduled run, or set a
[command approver](#choose-who-approves); a placement with neither stops
before renting and names the command. A rental's `github` row in
`lf machine logins` asks GitHub with the token the rental holds, so it reads
signed out as soon as a push would fail. `lf machine login github <rental>`
sends a fresh token for the repository whose checkout you run it from, in
place of the one the rental held.

| Placement stops with | Do this |
| --- | --- |
| The Loopflow app is not installed on the repository | Install it from the link printed, then place again; that placement asks for a new code |
| No approval for this repository and no terminal | `lf ondemand login github` in its checkout |
| The approving GitHub account is not the one `gh` is signed in to here | Approve as the `gh` account |

A rental signs in to GitHub for a GitHub origin and Linear for a Linear-backed
repository, and to nothing else unless the template's `--logins` names it:
`codex` or `opencode`. Each provider needs at most one approval. Claude uses
a forwarded token on each launch. Task placement requires either a forwarded
`CLAUDE_CODE_OAUTH_TOKEN` or `--logins codex` before renting.

Machines you own are unchanged: `lf machine login github mini` installs your
own `gh` token there.

### Per-run secrets

```bash
lf set --global forward.env '[CLAUDE_CODE_OAUTH_TOKEN, OPENAI_API_KEY]'
lf set --global forward.files '[~/.config/tool/token]'
```

| Setting | What a remote launch does |
| --- | --- |
| `forward.env` | Sets each named variable from this Machine's environment, else Doppler |
| `forward.files` | Copies each home-relative file to the same path, mode 0600 in a 0700 directory, when the destination has none |

Values and file contents travel on the SSH connection's input, never in
arguments, logs, the store or Session records. The forwarded environment is
read and its file removed before the remote command starts; one a failed launch
never read is removed by the next delivery. Only commands that may start an
agent carry anything. A destination that already has a named file keeps its
own, and the launch says so when its copy differs. A named variable with no
value, or a named file missing here, stops the launch, and an on-demand rental,
before anything is rented or sent. `CLAUDE_CODE_OAUTH_TOKEN` is the exception:
it is Claude's login, so without a value the launch goes without it and Claude
reads signed out there, while Codex launches run as usual. Loopflow does not read, choose among or
refresh what it forwards. Forward Claude's setup token, not its login files;
sign Codex in with `lf machine login codex`, not by forwarding `auth.json`.

Claude and Codex launches never see an exported `ANTHROPIC_API_KEY`,
`OPENAI_API_KEY` or `CODEX_API_KEY`, so a key in your shell cannot move them
onto API billing. Machine routing, repository setup and execution restrictions
still apply. See [Security](security.md).

Refresh configured `forward.files` on a live Machine with `lf machine forward`.

```bash
lf machine forward modal-1
lf machine forward modal-1 --task LOO-570
lf machine forward modal-1 --session session_…
```

The refresh replaces each named file atomically over SSH stdin and reads only
`forward.files`. With `--task` or `--session`, it then restarts the selected
Session's provider on its owning Machine, keeping the same conversation.
The provider keeps running if delivery fails or the refreshed login is unusable.

## Service authentication

```bash
lf machine login github
lf machine login linear --chrome-profile Work
lf machine login opencode mini
lf machine login opencode --api-key
```

The same command signs in GitHub, Linear, Asana, Doppler and OpenCode here;
GitHub, Linear and OpenCode also reach another Machine. Sending one to another Machine signs it in here
first if needed, then installs it there over SSH: a GitHub token, a Linear
access token refreshed here, or a fresh OpenCode device login. A destination
already signed in to a different account keeps its own. A rental gets a
narrower GitHub token than a Machine you own; see [Rentals](#rentals).
