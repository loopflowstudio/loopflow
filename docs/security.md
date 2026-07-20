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
lf ssh build-vm task pursue
```

`lf ssh` connects to an existing environment. It does not copy the repository
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

The same boundary covers subprocesses, repository hooks, MCP servers, plugins,
skills, browser tools, and commands an agent launches. Run repository
instructions and extensions as code from sources you trust.

Four controls answer different questions:

1. **Execution boundary:** which files, processes, and networks the OS permits.
2. **Action policy:** which vendor tools run automatically or ask first.
3. **Identity:** which accounts and external systems the process tree can use.
4. **Workflow review:** when kickoff, iterate, and gate sessions involve a
   human.

Only the first is general containment. Review and shipping decisions make work
legible; they do not contain hostile code.

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

## Understand account authority over SSH

### Do I have to be logged in on the remote machine?

No—not for foreground work. The machine where you type `lf ssh` is the
**origin**; the named machine is the **target**. The target `lf` can choose from
subscription accounts installed on either one:

```bash
# Offer the origin's accounts and include accounts installed on the target.
lf ssh my-company task pursue

# Prefer an account installed on the origin.
lf ssh --account jack@personal my-company task pursue

# Select from the target lf's combined local and forwarded catalog.
lf ssh my-company --account jack@company task pursue
```

The target does not need its own Claude or Codex login for the first two
commands. The account can exist only on your laptop and still run an agent on a
credential-free build box.

Work that continues after SSH returns is different:

```bash
lf start shipper                     # start it on this machine
lf ssh shipper-home start shipper    # run the same local operation there
```

The foreground control command can borrow origin authority. The resident it
starts cannot: it must use credentials and repository routes installed on the
machine where it keeps running. A foreground `--account` preference is not
durable configuration.

Wave `owner` and `home` fields are automatic-start policy, not an authorization
boundary. They stop a Home daemon from volunteering for somebody else's Wave;
a user who can write the repository or local registry can still run an
explicit `lf start <name>`.

See [Subscription Management](/docs/subscriptions) for connecting identities,
repository routes, account selectors, and the exact merged selection order.

### Where subscription credentials live

Each managed Claude or Codex identity has an owner-only provider home on the
machine where it was connected:

```text
~/.lf/accounts/claude/<account-id>/
~/.lf/accounts/codex/<account-id>/
```

That home contains the provider CLI's own authentication and session state.
Loopflow points local provider children at it with `CLAUDE_CONFIG_DIR` or
`CODEX_HOME`. Shared settings, skills, and plugins may be linked from the
normal `~/.claude` or `~/.codex` home; credentials and session state remain
isolated by account.

`~/.lf/loopflow.db` stores non-secret account metadata: verified login,
routing and credential state, health signals, repository routes,
access-profile bindings, and provider-session pins. These files and rows stay
on the machine that owns the identity. Loopflow has no central subscription
account service.

### What crosses SSH for a subscription account

`lf ssh` does not copy an account home, refresh credential, browser profile,
or database row. The origin runs a short-lived broker for the foreground SSH
process:

1. The broker advertises account identities, health facts, route preferences,
   and explicit outer selections. Advertising the catalog refreshes no OAuth
   credential.
2. The target combines that catalog with accounts installed locally.
3. When a provider launch chooses a forwarded account, the target asks the
   origin broker for that account.
4. The origin refreshes only the selected credential when necessary and sends
   one access token through an SSH-forwarded, owner-only socket.
5. The target passes that access token to the selected Claude or Codex child.
6. Health results and session pins return to the origin database that owns the
   identity.

The access token really does enter the target's process boundary. It is not put
in argv, logs, a remote account home, or a durable Loopflow store, but the
target `lf`, the provider child, and other code running as the same remote OS
user are inside the trust boundary while the broker lives. Use a dedicated
user, container, or VM when same-user code should not receive that authority.

The broker closes with the foreground SSH process. Its handle stops working,
temporary sockets are removed, and a surviving child cannot request another
token. Broker failure, an expired handle, a missing credential, an incompatible
remote `lf`, and a failed Home identity check all fail closed.

Nested `lf ssh` is rejected so borrowed authority cannot cross a second SSH
hop. Obvious detached forms such as `tmux`, `screen`, `nohup`, `systemd-run`,
and `--detach` are rejected when they would retain borrowed authority. Durable
Loopflow spawns scrub forwarded handles and singleton credentials before the
resident starts.

### What crosses SSH for other credentials

GitHub, Linear, and OpenCode Zen each have one effective credential for an
invocation rather than a routable catalog. `lf ssh` forwards the origin
credential automatically when one is available. If the origin does not provide
one, the target can use its native credential.

These singleton credentials are process-environment capabilities. Same-user
code on the target can read or retain them while present; they do not have the
subscription broker's lazy, per-account boundary. Loopflow-managed singleton
tokens in `~/.lf/loopflow.db` are encrypted at rest. The encryption key uses
the macOS Keychain or Linux secret service when available, with an owner-only
local file fallback.

SSH agent forwarding remains off unless requested:

```bash
lf ssh --forward-agent build-vm task pursue
```

Forward one Doppler secret by name:

```bash
lf ssh --secret SENTRY_AUTH_TOKEN build-vm release check
```

The Doppler CLI resolves the value on the origin. Only the requested value
crosses SSH. The Doppler master credential never does.

## Account for stored and transmitted data

Prompts, selected repository context, tool results, and conversation data go to
the configured model provider as part of an agent run. Browser tools, MCP
servers, GitHub, Linear, and other integrations receive the data sent to them
by their commands.

Loopflow keeps a local execution ledger plus prompt and conversation artifacts.
Trace directories are owner-only (`0700`) and artifact files are `0600`.
`lf trace --content` deliberately reads the exact captured prompt and
conversation. Provider or tool output can contain sensitive material, so treat
the trace store as sensitive even though it is local.

## Keep network services inside their intended boundary

`lfd` listens on `127.0.0.1` by default. It is not a remote multi-user identity
system; use SSH for remote operation. A non-loopback bind requires
`LF_LFD_ALLOW_NON_LOOPBACK=1`. Linear and GitHub webhook routes verify their
provider signatures. `/health` and `/status` are public on the bound interface,
so a non-loopback listener must sit behind a firewall or authenticating proxy.
Wave start and stop require a random per-process control capability stored in
the local endpoint record with owner-only permissions; the capability is never
logged or sent to agents.

The detached development fallback that `lf start` launches when no lfd service
is live is deliberately scrubbed. It can host Waves, but it does not retain
webhook secrets from the invoking shell. Install lfd as the Home service for
durable webhook ingress.

`lfd` keeps webhook secrets inside the Home server process. Its in-process
`WaveHost` listeners are trusted Loopflow control code, not agents. When they
spawn Wave bodies and provider processes, the durable boundary removes daemon
secrets along with forwarded SSH credentials. Agents do not inherit ingress
authority.

Repository instructions, skills, plugins, MCP servers, browser connections,
hooks, and installers can all extend what an agent can reach. Review their
source and configuration before adding them to a high-authority environment.
