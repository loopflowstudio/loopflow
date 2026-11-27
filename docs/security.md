---
layout: default
title: Security
---

# Security

Run trusted work directly on your laptop:

```bash
lf implement
```

Put unattended or untrusted work behind an OS boundary you control:

```bash
lf --machine build-vm implement
```

`lf --machine` runs inside an existing remote environment (SSH host, container,
or VM). That OS user, filesystem, network, and credential boundary determines
what the agent can reach.

## Choose the execution boundary first

Loopflow runs coding agents and subprocesses with the authority of the OS user
running `lf`. A Git worktree isolates branches, not security: interactive
Claude and Codex worktree sessions also get write access to the main repo's
`.git` metadata so Git commands work.

Four separate controls answer four different questions:

1. **Execution boundary:** which files, processes, and networks the OS permits.
2. **Action policy:** which vendor tools run automatically or ask first.
3. **Identity:** which accounts and external systems the process tree can use.
4. **Interactive steps:** where a Graph pauses for your decision.

Only the first is general containment. Review steps and approval workflows make
work legible; they do not contain hostile code, skills, hooks, or MCP servers.

## Effective action policy

Agents act without asking permission by default: Claude, Codex, OpenCode and
Antigravity skip every prompt, and Pi uses its own unsandboxed tools.
`yolo: false` brings back the vendor's own config and prompts in conversations
you attend. Unattended runs (`lf -b`, Task steps, auto, commit and PR text)
never ask, whatever `yolo` says. Either way the agent can reach anything the OS
user can; the execution boundary above is the only containment. See
[Configuration](config.md#permissions-yolo-and-worktree-access).

## Credentials over SSH

```bash
lf machine add mini
lf set --global forward.env '[CLAUDE_CODE_OAUTH_TOKEN]'
lf machine login codex mini
lf machine login github mini
lf machine login linear mini
lf --machine mini -a codex implement
```

- **Native logins:** Codex signs in on the destination through its own
  device login, run there over SSH with no terminal; only the approval page
  opens here ([Sign in on any Machine](subscriptions.md#sign-in-on-any-machine)).
  The login, and every refresh of it, stays on the destination. Claude reaches
  the destination as the `claude setup-token` you keep in Doppler and name in
  `forward.env`: it travels on SSH stdin with each launch, is set for that run
  only, and is never written there. Anyone who can read that launch's
  environment on the destination can use the token until it expires; trust the
  destination OS user with it. Loopflow does not choose a login, store a Claude
  or Codex token or refresh one. A Machine with no login is refused before
  launch with what fixes it. Only variables and home files named in personal
  `forward` settings are sent. Existing destination files are retained.
- **Service transfer:** OpenSSH verifies the host key and Loopflow checks the
  saved Machine identity before transfer. Credential bytes travel through SSH
  stdin, never in command arguments or logs, and land with owner-only
  permissions (`0600`). Linear tokens in SQLite are encrypted using an
  owner-only `provider-token.key`.
- **Rentals:** a rented Machine never receives your `gh` token. It gets a
  GitHub App user token that acts as you in the one repository it runs and
  expires 8 hours after issue. It can change `.github/workflows/`, so a
  rental can push a workflow that reads that repository's Actions secrets. The
  grant that renews it stays encrypted on your Machine. Giving back a
  repository's last rental ends the token then; a rental its provider deletes
  while your Machine is asleep leaves the token working until your Machine
  next checks, or the 8 hours pass. Codex, OpenCode and the forwarded Claude
  token are not narrowed for a rental: list in `--logins` only what the work
  needs. See [Rentals](subscriptions.md#rentals).
- **SSH agent forwarding:** SSH agent forwarding remains opt-in with
  `lf --machine <machine> --forward-agent`. A rental's first connection
  forwards neither the agent nor X11.

## Local client server

The local Loopflow server listens on a private Unix socket accessible only to
your OS user. Anyone admitted to a held terminal can type into it. Do not
expose this socket over a network proxy; use `lf server mobile` (Tailscale) for
authenticated phone access. See [Connect clients](server.md).

## Stored and transmitted data

- Prompts, context, and tool output go to your configured model provider (and
  to GitHub, Linear, Asana, or MCP servers when invoked).
- Local captures under `$LF_HOME/runs/` are owner-only (`0700` directories,
  `0600` files) and never uploaded to Linear, GitHub, or other Machines.
- Recursive Markdown under `scratch/` and repo/Wave `MEMORY.md` enter agent
  context automatically (symlinks are not followed). Keep secrets out of both
  and inject bridge tokens like `LF_DISCORD_TOKEN` through Doppler.
