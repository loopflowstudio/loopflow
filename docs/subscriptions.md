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
| Explicit model pin | Stop when that choice cannot continue |
| Headless order exhausted | Stop with the provider error and a next step |

Loopflow never waits for a subscription reset or retries through another saved
login. A quota reading or a successful answer mentioning limits does not trigger
fallback. After fixing native authentication or waiting for provider capacity,
retry explicitly. For a stopped Task, inspect `lf task status <ID>`, then run
`lf task run <ID>` to continue its remaining steps. To move a conversation to
another harness, use `lf -a codex resume <session-id>`.

## Remote Machines and rentals

```bash
ssh mini                         # sign in with claude or codex on mini
lf --machine mini -a codex implement
```

Install and authenticate the native client on the destination before unattended
work, including on rentals. The destination's environment and native login own
authentication. Missing authentication reaches the native client's login error.
Loopflow does not transfer Claude/Codex credentials, provision subscription
logins or renew rental subscription tokens. Machine routing, repository setup
and execution restrictions still apply. See [Security](security.md).

## Service authentication

```bash
lf account connect linear --chrome-profile Work
lf account connect github
lf account connect opencode
```

`lf account` retains authentication for GitHub, Linear, Asana, Notion, Doppler
and OpenCode. Service browser-profile preferences and connect/disconnect flows
remain separate from native Claude/Codex authentication.
