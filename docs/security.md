---
layout: default
title: Security
---

# Security

Run trusted personal work directly:

```bash
lf implement
```

Run the same `lf` on another machine:

```bash
lf ssh build-vm task pursue
```

`lf ssh` does not copy the repository or create a sandbox. It connects to the
target over SSH, enters the repository already present there, and runs the
target machine's `lf`. The remote OS user, container, or VM defines which
files, processes, and networks that work can reach.

## Do I have to be logged in on the remote machine?

No—not for a foreground `lf ssh` command. Either machine can hold the Claude or
Codex subscription login. The target `lf` sees both:

1. accounts installed on the target; and
2. accounts offered by the origin for this SSH invocation.

The **origin** is the machine where you type `lf ssh`. The **target** is the
machine named after it.

```bash
# Use the target's accounts plus every eligible account offered by this machine.
lf ssh my-company task pursue

# Prefer this exact account from the origin machine.
lf ssh --account jack@personal my-company task pursue

# Let the target lf resolve this selector from its combined account catalog.
lf ssh my-company --account jack@company task pursue
```

The target does not need its own subscription login for the first two commands.
The account can exist only on your laptop and still run an agent on a
credential-free build box. You do not log in twice, and Loopflow does not copy
the account's home directory to the build box.

Work that continues after the SSH command returns is different. A resident,
daemon, or detached process must use credentials installed on the machine where
it keeps running:

```bash
lf start shipper                     # start it on this machine
lf ssh shipper-home start shipper    # run the same local operation there
```

The second command may borrow origin authority while its foreground control
operation runs. The resident it starts belongs to `shipper-home`, so its
provider launches use accounts installed there. Borrowed accounts disappear
when the outer SSH process exits.

That is the practical rule:

- **Foreground remote work:** log in on the origin, the target, or both.
- **Long-running remote work:** log in on the machine that owns the process.

## What an agent identity stores

`lf auth connect` does not create a Claude or Codex account. It registers an
existing provider login as an isolated Loopflow agent identity:

```bash
lf auth connect claude jack@example.com --chrome-profile jack@example.com
lf auth connect codex work@example.com --chrome-profile work@example.com
```

Each identity has two local parts.

The provider home holds the provider CLI's own authentication and session
state:

```text
~/.lf/accounts/claude/<account-id>/
~/.lf/accounts/codex/<account-id>/
```

For example, Codex keeps `auth.json` in its account home and Claude keeps its
scoped credential state there. Loopflow points a local provider child at that
home with `CODEX_HOME` or `CLAUDE_CONFIG_DIR`. Account homes are owner-only
directories. Shared provider settings, skills, and plugins may be linked from
the normal `~/.codex` or `~/.claude` directories; the login and session state
remain isolated per account.

The Loopflow database holds the facts used to choose that home:

```text
~/.lf/loopflow.db
```

Those facts include the provider and verified login, credential and routing
state, health and usage signals, repository routes, access-profile bindings,
and provider-session pins. A session pin records which account created a
provider session so resuming it does not silently switch identities.

An access profile is a pointer to a browser profile that can authenticate the
account. A repository account route is an ordered list of subscription logins
to try for one provider in one repository. It is selection metadata, not an SSH
or network route, and it contains no credential:

```bash
lf route set codex work@ personal@
lf route show
```

Both the account home and these database records belong to the machine where
the identity was registered. Loopflow does not synchronize them through a
central account service.

## How account selection works over SSH

`lf ssh` runs only a remote `lf`, so the target name is the command boundary.
Options before the target apply to the origin-side SSH invocation. Everything
after the target is ordinary syntax for the target's `lf`:

```bash
# Origin-side preference: jack@personal must be installed here.
lf ssh --account jack@personal my-company implement

# Target-side preference: resolve jack@company from the combined catalog.
lf ssh my-company --account jack@company implement
```

There is no explicit `-- lf` and no arbitrary remote command. Use ordinary
`ssh` for arbitrary programs. Nested `lf ssh` is rejected so borrowed authority
cannot be forwarded through an unbounded chain of machines.

The target may be an SSH hostname or a Loopflow Home ID. A Home ID resolves to
that Home's current SSH address and makes the remote `lf` prove which Home it
is. This machine lookup is unrelated to a repository account route, which only
orders subscription logins.

Without an origin-side selector, Loopflow offers every connected managed Claude
and Codex identity on the origin, along with the facts that say whether each is
eligible for automatic use. It also forwards the origin repository's account
routes. The target merges those with its own accounts and its own route for the
checked-out repository.

Selection proceeds in this order:

1. target-side `--account` preferences;
2. origin-side `--account` preferences;
3. the target repository route;
4. the forwarded origin repository route; and
5. the remaining eligible target and forwarded accounts.

Target-local accounts come before equivalent forwarded accounts when no
explicit preference distinguishes them. Missing, disabled, cooling, or limited
accounts are skipped. A resumed provider session stays pinned to the local or
forwarded identity that created it.

`--account` prefers a login and leaves the rest of the route as fallback.
`--only-account` restricts the process tree to the selected logins. An
origin-side `--only-account` is resolved against origin identities before SSH
connects; the target may narrow that selection but cannot widen it. The two
flags cannot be combined.

```bash
lf ssh --only-account codex=jack@personal my-company review
lf ssh my-company --only-account claude=jack@company review
```

Account commands show where each candidate comes from. Forwarded entries are
read-only: authenticate, disconnect, and edit routes on the machine that owns
the identity.

## What crosses SSH for a subscription account

Forwarding an account does not copy its provider home, refresh credential,
browser profile, or database rows. The origin runs a short-lived account broker
for the foreground SSH process.

1. The broker advertises the origin account catalog, health facts, repository
   route, and explicit outer selection. This does not refresh every OAuth
   credential.
2. The target `lf` combines that catalog with accounts installed locally.
3. When a provider launch selects a forwarded account, the target asks the
   origin broker for that account.
4. The origin refreshes only the selected credential when necessary and sends
   one access token through the SSH-forwarded, owner-only broker socket.
5. The target passes that access token to the selected Claude or Codex child.
6. Health results and session pins return to the origin, which records them in
   the database that owns the identity.

The access token really does enter the target's process boundary. It is not put
in argv, logs, a remote account home, or a durable Loopflow store, but the
target `lf`, the provider child, and other code running as the same remote OS
user are inside the trust boundary while the broker lives. Use a dedicated
user, container, or VM when same-user code should not receive that authority.

The broker closes with the foreground `lf ssh` process. Its handle and socket
stop working, temporary sockets are cleaned up, and a surviving child can no
longer request another token. Broker failure, an expired handle, a missing
credential, an incompatible remote `lf`, or a failed Home identity check fails
closed.

Obvious detached forms such as `tmux`, `screen`, `nohup`, `systemd-run`, and
`--detach` are rejected when they would retain borrowed authority. Commands
that intentionally create durable Loopflow residents scrub forwarded account
handles and singleton credentials before the resident starts.

## What crosses SSH for other credentials

GitHub, Linear, and OpenCode Zen each have one effective credential for an
invocation rather than a routable catalog of subscription accounts. `lf ssh`
forwards the origin credential automatically when one is available; there is
no GitHub or Linear account selector to repeat on every command. If the origin
does not provide one, the target can use its native credential.

These singleton credentials are process-environment capabilities. Same-user
code on the target can read or retain them while present; they do not have the
subscription broker's lazy, per-account boundary. Loopflow-managed singleton
tokens stored in `~/.lf/loopflow.db` are encrypted at rest. The encryption key
uses the macOS Keychain or Linux secret service when available, with an
owner-only local file fallback.

SSH agent forwarding remains off unless requested:

```bash
lf ssh --forward-agent build-vm task pursue
```

Forward a Doppler secret by name:

```bash
lf ssh --secret SENTRY_AUTH_TOKEN build-vm release check
```

The Doppler CLI resolves the value on the origin. Only the requested value
crosses SSH; the Doppler credential itself does not.

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

`lfd` listens on `127.0.0.1` by default and uses a machine-local capability
token. It is not a remote multi-user identity system. Use SSH for remote
operation. A non-loopback bind is an explicit local-network experiment and
requires a bearer token; see [lfd](/docs/lfd).

Repository instructions, skills, plugins, MCP servers, browser connections,
hooks, and installers can all extend what an agent can reach. Review their
source and configuration before adding them to a high-authority environment.
