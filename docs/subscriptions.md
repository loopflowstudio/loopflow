---
layout: default
title: Native authentication
---

# Native authentication

Sign in with the native client, then launch work through Loopflow:

```bash
lf machine login claude         # runs Claude's own sign-in
lf -a claude implement

lf machine login codex          # runs Codex's own sign-in
lf -a codex implement
```

Signing in with `claude` or `codex login` yourself works the same. Each launch
uses the native client's authentication on the Machine where it runs. Claude and Codex own login, logout, credential refresh and subscription
usage limits. Their API-key configuration also applies. Loopflow adds context,
records Sessions and usage, and continues work across configured models and
harnesses.

## Homes and existing conversations

Claude uses its native home (`~/.claude`, or `CLAUDE_CONFIG_DIR`); Codex uses
`~/.codex`, or `CODEX_HOME`. Loopflow honors those environment settings and
uses the native client's configured authentication. Changing a native
login affects subsequent launches; an already running client follows its own
authentication behavior.

```bash
lf resume <session-id>
```

Resume retains the native conversation ID. Older conversations stored outside
the native home are adopted as transcript history only. Existing native history takes precedence; newer transcript data
is retained. Credentials are never copied as part of resume. Missing history
is reported instead of silently starting an empty conversation.

Usage records retain provider-reported tokens and costs. Native clients own
subscription limits and authentication; Loopflow does not poll quotas or change
logins.

## When a provider refuses work

```bash
lf set --global agents claude,codex
lf -b implement
```

A classified usage-limit or model-unavailable refusal lets a Graph step try
its configured next model or harness. Switching harnesses seeds the new
conversation with earlier history and retains earlier outcomes. See
[agent order](config.md#keep-going-when-a-provider-runs-out) for defaults,
model fallthrough and explicit pins.

| Launch | After a classified refusal |
| --- | --- |
| Graph step, with or without a Task | Follow configured model/harness fallbacks |
| Other headless work (`lf -b`, commit and PR text) | Stop with the provider error; someone is waiting on it |
| Interactive, with or without a Task | Keep the provider, thread and refusal visible; wait for an explicit retry or move |
| Explicit agent | Keep the harness; bare Claude retains model fallthrough |
| Explicit model pin | Wait on that choice, then stop |
| Graph step's order exhausted on a usage limit | Wait 5, 15, 30, then 60 minutes, trying the order again after each; then stop with the provider error and a next step |
| Revoked or missing login | Skip that provider's other models; with no other provider left, stop with the `lf machine login` command that fixes it |
| Order exhausted otherwise | Stop with the provider error and a next step |

```
claude: You've hit your limit. Resets 3pm (America/Los_Angeles).
Waiting until Nov 16 14:05 before trying again
```

Each try after a wait starts the order again at its first agent and uses
whatever native login the Machine has then, so signing in elsewhere or buying
more usage is picked up. `lf task status <ID>` shows the wait and when the
next try starts. Loopflow never polls quotas, reads reset times or retries
through another saved login. A quota reading or a successful answer mentioning
limits does not trigger fallback. After fixing native authentication, retry a
stopped run explicitly. For a stopped Task, inspect `lf task status <ID>`, then
run `lf task run <ID>` to continue its remaining steps. To move a conversation to
another harness, use `lf -a codex resume <session-id>`.

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
names `CLAUDE_CODE_OAUTH_TOKEN`. Each row also shows the provider's last
refusal there, login missing or usage limit, and when. `lf machine logout` runs
each provider's own logout.

Every launch, local or remote, asks the same clients first, with the launch's
environment, so a forwarded token counts as signed in. A Machine with no
login for a provider skips that provider, like a [revoked login](#when-a-provider-refuses-work),
and with none left stops before an agent starts:

```
mini has no Codex login; run `lf machine login codex mini`
mini has no Claude login; forward CLAUDE_CODE_OAUTH_TOKEN (`lf set --global forward.env`)
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
GitHub, Linear, Asana, Notion and OpenCode. Claude's own login here opens its
page itself. `--chrome-profile <profile>` overrides the setting for one run.
The browser picks the account; Loopflow keeps no list of accounts. A command
approver needs no terminal, so unattended sign-ins use one.

### Rentals

```bash
lf ondemand add code --provider daytona --vcpus 4 --memory-gb 8 --max 2 \
  --max-usd-per-hour 0.40 --logins codex,github
lf task run LOO-570 --rent code
```

A rented Machine is signed in as it is prepared, at most one approval per
provider: to the template's `--logins`, or with none set, to every provider
signed in here that reaches another Machine (Codex, GitHub, Linear,
OpenCode). With command approvers set, no one has to be there. Claude needs no
sign-in at prepare: each launch on the rental carries the forwarded token. A
rental with neither, no forwarded `CLAUDE_CODE_OAUTH_TOKEN` and no Codex to
sign in, stops before anything is rented.

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
before anything is rented or sent. Loopflow does not read, choose among or
refresh what it forwards. Forward Claude's setup token, not its login files;
sign Codex in with `lf machine login codex`, not by forwarding `auth.json`.

Claude and Codex launches never see an exported `ANTHROPIC_API_KEY`,
`OPENAI_API_KEY` or `CODEX_API_KEY`, so a key in your shell cannot move them
onto API billing. Machine routing, repository setup and execution restrictions
still apply. See [Security](security.md).

## Service authentication

```bash
lf machine login github
lf machine login linear --chrome-profile Work
lf machine login opencode mini
lf machine login opencode --api-key
```

The same command signs in GitHub, Linear, Asana, Notion, Doppler and OpenCode,
here or on another Machine. Sending one to another Machine signs it in here
first if needed, then installs it there over SSH: a GitHub token, a Linear
access token refreshed here, or a fresh OpenCode device login. A destination
already signed in to a different account keeps its own.
