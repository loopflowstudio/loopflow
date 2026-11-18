---
layout: default
title: Security
---

# Security

Run trusted personal work directly:

```bash
lf implement
```

Put unattended or untrusted work behind an OS boundary you control:

```bash
lf --machine build-vm implement
```

`lf --machine` connects to an existing environment. It does not copy the repository
or create a sandbox around the target. The remote OS user, container, or VM
defines which files, processes, networks, and credentials the work can reach.

## Choose the execution boundary first

Loopflow launches vendor agents and ordinary host processes with the authority
of the user running `lf`. The useful boundary is therefore a laptop account, a
dedicated Unix user, a container, or a VM. Apply filesystem mounts, network
rules, process limits, and credential access at that boundary.

A Git worktree separates changes but is not a security sandbox. Claude and
Codex worktree sessions also receive write access to the main repository's Git
metadata so normal Git operations work.

Task Graphs retain failed commands as tool output. A failed command blocks
handoff only when its output contains a permission, filesystem, network refusal,
or missing executable diagnostic. A nonzero exit, missing search path, or quoted
denial string in source output does not establish a blocked capability. The
blocker names the matching diagnostic so the failure can be corrected.

The same boundary covers subprocesses, repository hooks, MCP servers, plugins,
skills, browser tools, and commands an agent launches. Run repository
instructions and extensions as code from sources you trust.

Four controls answer different questions:

1. **Execution boundary:** which files, processes, and networks the OS permits.
2. **Action policy:** which vendor tools run automatically or ask first.
3. **Identity:** which accounts and external systems the process tree can use.
4. **Interactive steps:** where a graph stops for your decision.

Only the first is general containment. Review and shipping decisions make work
legible; they do not contain hostile code.

## Know the effective action policy

Loopflow chooses an automation floor for headless work only:

- Non-interactive Codex runs get `workspace-write` and approval policy `never`.
- Non-interactive Claude runs skip permission prompts.
- Non-interactive OpenCode keeps its configured permissions; Loopflow answers
  each permission ask for the running turn.
- Pi uses its own tool policy and credentials; it provides no built-in sandbox.
- More-permissive vendor configuration remains more permissive.
- Less-permissive vendor configuration is raised to this floor; Claude prints a
  warning when that happens.

Interactive launches send no permission settings, so the vendor's own
configuration and approval prompts apply. `yolo: true` selects the vendor's full
bypass mode; for Codex that also disables its sandbox. See
[Configuration](/docs/config#yolo) for the exact vendor flags.

These modes govern vendor tool behavior. They do not narrow the OS user,
network, browser, MCP, or credential boundary around the whole process tree.

Skills can edit, commit, push, land a PR, deploy, or update GitHub and Linear
when the matching tool and credential are present. A gate may decide that work
is ready to ship. `lf pr submit` leaves the exact-head merge to a person;
`land` requests GitHub auto-merge. GitHub remains authoritative for
whether a PR merged. Release authority is separate and depends on the target's
configured publisher workflow and credentials.

## Understand credential authority over SSH

```bash
lf machine add mini
lf machine connect mini github
lf machine connect mini opencode
lf machine connect mini linear
ssh mini                         # sign in with the native Claude/Codex client
lf --machine mini -a codex implement
```

Claude and Codex use destination-native authentication. A Machine can keep its
own login; unattended work can receive explicitly named environment variables
or home files through [named forwarding](subscriptions.md#remote-machines-and-rentals).
Loopflow does not choose a login, interpret those files or refresh them. Existing
destination files are retained. Missing authentication produces the native
client's error.

Service credential transfer remains explicit. OpenSSH checks the known host key
and Loopflow checks the saved Machine identity before transfer. Linear receives a
fresh OAuth grant. OpenCode Console uses a fresh device grant; an OpenCode API
key is copied, like a GitHub token. Trust the destination OS user with any
credential installed there. Credential bytes travel through SSH stdin, never
command arguments or diagnostic output. Native files use owner-only permissions.

Linear credentials in SQLite are encrypted. The encryption key lives in an
owner-only `provider-token.key` file. An existing platform key is retained before
use; a locked or denied Keychain read cannot replace a key needed by existing
encrypted credentials. Unlock that Keychain once to retain its key. Subsequent
SSH use reads the private file. A fresh credential-free machine can create its
own file key without an unlocked Keychain.

SSH agent forwarding remains opt-in with `lf --machine <machine> --forward-agent`.
Automatic ambient-token forwarding is disabled. Only variables and home files
named in personal `forward` settings are sent; resolve variable values from the
initiator's environment or Doppler. Names, rather than values, belong in settings.
Do not name a broad Doppler credential when a scoped service token is sufficient.

## Keep client access separate from Machine logins

The local Loopflow server accepts the same OS user through a private Unix socket.
It exposes existing Work operations and bridges views of held terminals. Any
connection the server admits to a held terminal can type into it: access is
granted per person and Machine, not per view, and there are no input claims to
take or hold.

Do not expose this socket through a network proxy: the proxy's local identity
cannot authenticate a remote client. Authenticated phone access is separate
work, with its own client credentials. Adding an SSH Machine grants an execution
destination, independently of client access. Installation owns server registration
and respects disabled background items. See [Connect clients](server.md).

## Account for stored and transmitted data

Prompts, selected repository context, tool results, and conversation data go to
the configured model provider as part of an agent run. Browser tools, MCP
servers, GitHub, Linear, and other integrations receive the data sent to them
by their commands.

Loopflow keeps Machine-local captured inputs and conversation history with prompt, conversation, and raw
provider evidence under `$LF_HOME/runs/`. Bundle directories are owner-only
(`0700`) and artifact files are `0600`. Provider or tool output can contain
sensitive material, so treat the Machine store and payloads as sensitive even though it is
local. The bundles are not uploaded to Linear, GitHub, or another Loopflow
Machine. Reading another Machine with `lf --machine <machine-id> session history` executes the read on
that machine.

## Keep bridge credentials private

`lf discord serve` makes outbound requests and launches bounded conversations. Inject
`LF_DISCORD_TOKEN` through Doppler; provider children do not inherit it. No Wave
or Machine HTTP service is required. Use `lf --machine` for remote operation.

Repository instructions, skills, plugins, MCP servers, browser connections,
hooks, and installers can all extend what an agent can reach. Review their
source and configuration before adding them to a high-authority environment.

## Automatic context

Recursive Markdown under `scratch/` and repository `MEMORY.md` are agent input,
including local working notes. Keep private material outside these context
sources. Automatic scratch traversal and repository memory do not follow
symlinks; explicit document requests are separate. This is not a secret scanner
or a restriction on what the provider can read using its own tools.
