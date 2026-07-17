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
# The remote user, container, or VM owns the filesystem and network boundary.
lf ssh agent@build-vm -- lf task pursue
```

For sensitive infrastructure, combine that boundary with narrow mounts, network
policy, and scoped credentials. `lf ssh` enters the environment; it does not
create a sandbox around it.

Four controls answer different questions:

1. **Execution boundary:** which files, processes, and networks the OS permits.
2. **Action policy:** which vendor tools run automatically or ask first.
3. **Identity:** which accounts and external systems the process tree can use.
4. **Workflow review:** when kickoff, iterate, and gate sessions involve a human.

Only the first is general containment. Review and shipping decisions make work
legible; they do not contain hostile code.

## Choose the execution boundary

Loopflow launches vendor agents and ordinary host processes with the authority
of the user running `lf`. The useful boundary is therefore a laptop account, a
dedicated Unix user, a container, or a VM. Apply filesystem mounts, network
rules, process limits, and credential access at that boundary.

A Git worktree separates changes but is not a security sandbox. Claude and
Codex worktree sessions also receive write access to the main repository's Git
metadata so normal Git operations work.

The same boundary covers subprocesses, repository hooks, MCP servers, plugins,
skills, browser tools, and commands an agent launches. Run repository
instructions and extensions as code from sources you trust.

## Know the effective action policy

Loopflow chooses an automation floor for headless work:

- Codex gets `workspace-write`; non-interactive runs also get approval policy
  `never`.
- Non-interactive Claude runs skip permission prompts.
- Non-interactive OpenCode receives `permission: allow`.
- More-permissive vendor configuration remains more permissive.
- Less-permissive vendor configuration is raised to this floor with a warning.

Interactive launches retain the vendor's interactive approval behavior where
possible. `yolo: true` selects the vendor's full bypass mode; for Codex that
also disables its sandbox. See [Configuration](/docs/config#yolo) for the exact
vendor flags.

These modes govern vendor tool behavior. They do not narrow the OS user,
network, browser, MCP, or credential boundary around the whole process tree.

Skills can edit, commit, push, land a PR, deploy, or update GitHub and Linear
when the matching tool and credential are present. A gate may decide that work
is ready to ship. The LLM session still performs the merge or release action;
the GitHub merge button is not a separate lifecycle authority.

## Select local accounts for remote work

Prefer an account without removing the normal provider route:

```bash
lf --account reserve ssh mini -- lf task pursue

lf --account claude=personal \
  --account codex=reserve \
  ssh mini \
  -- lf task pursue
```

`--account` tries each matching managed account once before that provider's
normal local route. An unqualified selector matches account IDs or login emails
independently for Claude and Codex. Providers with no match keep their normal
routes.

Restrict the process tree to exact accounts:

```bash
lf --only-account claude=personal \
  --only-account codex=reserve \
  ssh mini \
  -- lf task pursue
```

`--only-account` removes every unselected account. A provider without a
selected account is unavailable. The two flags cannot be combined. The same
flags apply to local invocations:

```bash
lf --account codex=reserve implement
lf --only-account claude=personal review
```

### What crosses SSH

Loopflow resolves managed Claude and Codex accounts on the origin machine. The
remote process receives an opaque, short-lived lease handle. SSH
reverse-forwards an owner-only Unix socket to a broker owned by the foreground
`lf ssh` process. A provider launch receives only its selected credential.

When no managed route exists, `lf ssh` preserves the ambient-login fallback:
it forwards the extracted Claude or Codex access token in the remote process
environment. Configure a managed route or pass `--only-account` when provider
authority must stay behind the broker.

Nested `lf` processes inherit the same fixed grant through the handle. They
cannot widen, narrow, or reorder it; nested `--account` and `--only-account`
flags fail. A resumed provider session stays on its recorded account when that
account belongs to the grant.

Broker failure, a missing credential, and an expired handle fail closed. During
a lease, remote managed accounts and routes are not consulted. `lf auth
accounts` and `lf route show` display only the forwarded view. Access-profile
inspection and all authentication, account, and route edits are rejected.
`lf usage` skips subscription polling and still reports process token spend.
Before the target command starts, the remote `lf` proves it understands the
lease and can reach the broker; an incompatible binary fails closed.

The broker closes with the foreground SSH process. Common detached forms such
as `tmux`, `screen`, `nohup`, `systemd-run`, and `--detach` are rejected, and a
lease inherited by a child cannot be re-forwarded over SSH. Put `lf ssh` on the
outer account-selected invocation; this also rules out a second SSH hop. A
background process that survives the outer command loses broker access when SSH
exits. Provider credentials are not placed in argv, logs, remote account homes,
keychains, or durable Loopflow stores. Temporary sockets are removed during
normal cleanup.

The remote host and processes running as the same remote OS user are inside the
lease trust boundary. They can use its handle while it lives. Use a dedicated
user, container, VM, or credential-free home when same-user code must not reach
ambient remote credentials.

## Forward other credentials deliberately

`lf ssh` forwards the current GitHub credential for HTTPS Git operations and the
managed Linear credential used by `lf pm`. They are process-environment
capabilities available inside the remote user boundary; same-user code can copy
or retain them. SSH agent forwarding stays off unless `--forward-agent` is
explicit.

Forward one Doppler secret by name:

```bash
lf ssh mini --secret SENTRY_AUTH_TOKEN -- lf release check
```

The Doppler CLI resolves that value on the origin machine. Only the requested
secret crosses SSH. The Doppler master credential never does.

Managed provider tokens are encrypted in the local store. The key uses the
macOS Keychain or Linux secret service when available, with an owner-only local
file fallback.

## Account for stored and transmitted data

Prompts, selected repository context, tool results, and conversation data go to
the configured model provider as part of an agent run. Browser tools, MCP
servers, GitHub, Linear, and other integrations receive the data sent to them by
their commands.

Loopflow keeps a local execution ledger plus prompt and conversation artifacts.
Trace directories are owner-only (`0700`) and artifact files are `0600`.
`lf trace --content` deliberately reads the exact captured prompt and
conversation. Provider or tool output can contain sensitive material, so treat
the trace store as sensitive even though it is local.

## Keep network services inside their intended boundary

`lfd` listens on `127.0.0.1` by default and uses a machine-local capability
token. It is not a remote multi-user identity system. Use SSH for remote
operation. A non-loopback bind is an explicit local-network experiment and
requires a bearer token; see [lfd](/docs/lfd).

Repository instructions, skills, plugins, MCP servers, browser connections,
hooks, and installers can all extend what an agent can reach. Review their
source and configuration before adding them to a high-authority environment.
