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

```bash
lf machine login claude mini
#   Approve Claude for mini in Chrome (Work), then paste the code it shows:
#   > ****
#   mini: Claude signed in as jack@example.com (max)
lf machine login codex mini
#   Approve Codex for mini in Chrome (Work) with code ABCD-1234
#   mini: Codex signed in (ChatGPT)
lf --machine mini -a codex implement
```

`lf machine login <provider> [<machine>…]` runs the provider's own login on
each named Machine, or on this one when none is named, and opens its approval
page here. Claude runs `claude auth login`; you approve, and paste the code the
page shows. Codex runs `codex login --device-auth`; you approve with the code
it prints, within 15 minutes. Each Machine ends up with its own login, written
and refreshed by the unmodified client. No credential crosses the network: the
only thing sent is Claude's one-time code, which works only with a verifier
that never leaves the target's `claude` process. Loopflow stores no Claude or
Codex token.

Codex's device code login must be enabled in ChatGPT security settings. Running
`lf machine login` again replaces that Machine's login with whichever account
approves it.

```bash
lf machine logins mini
#   claude    signed in    jack@example.com (max)
#   codex     signed in    ChatGPT
#   github    signed in    jack-heart
#   linear    signed out
lf machine logout codex mini
```

`lf machine logins [<machine>] [--json]` asks the Machine's own clients
(`claude auth status`, `codex login status`) and shows `unknown` when one can't
answer. It also shows each provider's last refusal there, login missing or
usage limit, and when. `lf machine logout` runs each provider's own logout.

Every launch, local or remote, asks the same clients first. A Machine with no
login for a provider skips that provider, like a [revoked login](#when-a-provider-refuses-work),
and with none left stops before an agent starts:

```
mini has no Codex login; run `lf machine login codex mini`
```

An unknown answer launches anyway. To sign in again wherever a provider is
signed out, or refused a launch with none of that provider since:

```bash
lf machine login claude --refused
```

### Choose who approves

```bash
lf set --global approvers.claude.chrome Work
lf set --global approvers.codex.command 'my-approver'
```

| `approvers.<provider>` | What a sign-in does |
| --- | --- |
| unset | Opens the default browser |
| `chrome: <profile>` | Opens that Chrome profile, by name or directory |
| `command: <cmd>` | Runs the command with `LF_APPROVE_PROVIDER`, `LF_APPROVE_URL`, `LF_APPROVE_MACHINE` and, when the page asks for a device code (Codex, GitHub, OpenCode), `LF_APPROVE_CODE`; exit 0 means approved, and for Claude its first output line is the code |

The setting covers every provider whose sign-in opens a page: Claude, Codex,
GitHub, Linear, Asana, Notion and OpenCode. `--chrome-profile <profile>`
overrides it for one run. The browser picks the account; Loopflow keeps no list
of accounts. Pasting Claude's code needs a terminal; a command approver doesn't,
so unattended sign-ins use one.

### Rentals

```bash
lf ondemand add code --provider daytona --vcpus 4 --memory-gb 8 --max 2 \
  --max-usd-per-hour 0.40 --logins claude,github
lf task run LOO-570 --rent code
```

A rented Machine is signed in as it is prepared, at most one approval per
provider: to the template's `--logins`, or with none set, to every provider
signed in here. With command approvers set, no one has to be there.

### Per-run secrets

```bash
lf set --global forward.env '[OPENAI_API_KEY]'
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
refresh what it forwards. Sign Claude and Codex in with `lf machine login`, not
by forwarding their login files.

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
