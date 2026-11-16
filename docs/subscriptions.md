---
layout: default
title: Native authentication
---

# Native authentication

Sign in with the native client, then launch work through Loopflow:

```bash
claude                          # complete Claude's sign-in
lf -a claude implement

codex login                     # sign in to Codex
lf -a codex implement
```

Each launch uses the native client's authentication on the Machine where it
runs. Claude and Codex own login, logout, credential refresh and subscription
usage limits. Their API-key configuration also applies. Loopflow adds context,
records Sessions and usage, and continues work across configured models and
harnesses.

## Homes and existing conversations

Claude uses its native home (`~/.claude`, or `CLAUDE_CONFIG_DIR`); Codex uses
`~/.codex`, or `CODEX_HOME`. Loopflow honors those environment settings and does
not replace the login with a saved subscription credential. Changing a native
login affects subsequent launches; an already running client follows its own
authentication behavior.

```bash
lf resume <session-id>
```

Resume retains the native conversation ID. Older conversations stored under
Loopflow-managed account homes are adopted into the native home as transcript
history only. Existing native history takes precedence; newer transcript data
is retained. Credentials are never copied as part of resume. Missing history
is reported instead of silently starting an empty conversation.

Existing saved accounts, credentials and historical account labels remain stored.
They do not select a login or establish its current capacity. New usage records
retain provider-reported tokens and costs; subscription identity stays unknown
unless the native provider reports it. Loopflow does not poll subscription
quotas, switch logins or redeem resets. Old `account` preferences have no effect
on launches; `--account` and `--only-account` are removed.

## When a provider refuses work

```bash
lf set --global agents claude,codex
lf -b implement
```

A classified usage-limit or model-unavailable refusal lets a headless run try
its configured next model or harness. Switching harnesses seeds the new
conversation with earlier history and retains earlier outcomes. See
[agent order](config.md#keep-going-when-a-provider-runs-out) for defaults,
model fallthrough and explicit pins.

| Launch | After a classified refusal |
| --- | --- |
| Headless, with or without a Task | Follow configured model/harness fallbacks |
| Interactive, with or without a Task | Keep the provider, thread and refusal visible; wait for an explicit retry or move |
| Explicit agent | Keep the harness; bare Claude retains model fallthrough |
| Explicit model pin | Wait on that choice, then stop |
| Headless order exhausted on a usage limit | Wait 5, 15, 30, then 60 minutes, trying the order again after each; then stop with the provider error and a next step |
| Headless order exhausted otherwise | Stop with the provider error and a next step |

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

## Remote Machines and rentals

```bash
ssh mini                         # sign in with claude or codex on mini
lf --machine mini -a codex implement
```

A Machine you own can keep its own native login. An on-demand rental has none,
so name what its launch should carry:

```bash
claude setup-token                                       # once, on this Machine
doppler secrets set CLAUDE_CODE_OAUTH_TOKEN > /dev/null  # paste the token
lf set --global forward.env '[CLAUDE_CODE_OAUTH_TOKEN]'
lf set --global forward.files '[~/.codex/auth.json]'
```

| Setting | What a remote launch does |
| --- | --- |
| `forward.env` | Sets each named variable from this Machine's environment, else Doppler |
| `forward.files` | Copies each home-relative file to the same path, mode 0600 in a 0700 directory, when the destination has none |

Values and file contents travel on the SSH connection's input, never in
arguments, logs, the store or Session records. The forwarded environment is
read and its file removed before the remote command starts. A destination that
already has a named file keeps its own, and the launch says so. A named
variable with no value, or a named file missing here, stops the launch, and an
on-demand rental, before anything is rented or sent. Loopflow does not read,
choose among or refresh what it forwards.

Claude: `claude setup-token` creates a long-lived token for scripts; forward it
as `CLAUDE_CODE_OAUTH_TOKEN`. An `ANTHROPIC_API_KEY` forwards the same way and
bills at API rates.

Codex on a Plus or Pro plan: forward `~/.codex/auth.json`, the login file the
[Codex authentication docs](https://learn.chatgpt.com/docs/auth) describe
copying to a headless machine. Treat it like a password. On this Mac, Codex
0.169.0 keeps its login in that file; a Codex configured to keep credentials in
the OS credential store has no file to copy. Copies of one login share its
refresh credential, so one copy refreshing can sign the others out; use a
separate login, `codex login --device-auth` on the destination, or an API key
(`printenv OPENAI_API_KEY | codex login --with-api-key`, billed at API rates)
when that matters. Codex access tokens exist only for ChatGPT Business and
Enterprise workspaces whose owner allows them; there, forward
`CODEX_ACCESS_TOKEN`, which Codex reads from its environment without
`codex login`.

Without forwarding, install and authenticate the native client on the
destination before unattended work. Machine routing, repository setup and
execution restrictions still apply. See [Security](security.md).

## Service authentication

```bash
lf account connect linear --chrome-profile Work
lf account connect github
lf account connect opencode
```

`lf account` retains authentication for GitHub, Linear, Asana, Notion, Doppler
and OpenCode. Service browser-profile preferences and connect/disconnect flows
remain separate from native Claude/Codex authentication.
