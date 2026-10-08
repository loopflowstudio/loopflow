# Work on another machine

Status: LOO-411 implements step 2, the machine record, in PR #1489.
Parent PR #1484 merged as `626789dcd`; supported sync integrated main after the
first republication attempt rejected the stale stacked base.
Jack Heart authorized implementation and publication on 2026-10-07, then stopping
for review. The rename is inherited; later remote-work slices remain intent.

## Remaining for LOO-411 (global selector, 2026-10-07)

Jack Heart's latest decision (`c597174a-6330-4be3-9509-3e7667caf7d3`)
replaces the SSH command with `lf --machine <label-or-id> <command…>`.
The whole command, minus transport options, runs in the saved remote repository.
There is no per-call repository override or command allowlist. Task, worktree,
Wave, help and account options belong to that invocation. Account preferences
resolve against the target's combined catalog, regardless of argument order;
inherited origin restrictions still constrain its foreground grant.
`--secret` and `--forward-agent` require `--machine` and are consumed by transport.
`lf --machine Y machine add X` adds X to Y's registry.

The private shared OpenSSH connections, interactive first-install offer and
specific recovery diagnostics remain. The selector and focused verification are
complete, including the compression checkpoint `b347fe4d0`. Remaining for this
slice: the affected gate, investigation of the retained open-output-handle
warnings, disposable-account published-installer proof, and republication of
PR #1489 for review. Real broker cleanup and configured SSH/account continuity
remain unproved by the simulated transport and earlier loopback test. Jack's
latest comment requires stopping after republication, without landing. Later
slices remain intent; their Codex holder and shell-pane choices are unchanged.

Jack Heart prohibited copying code, tests, help text or config from herdr or cmux
on 2026-10-07; their described behaviour informs an independent implementation.

## What to build

Someone with a second machine names it once, starts a Task there from the
laptop, closes the laptop, and returns to the same work from the laptop, the
Mac app or a phone.

## Placement

Infrastructure Wave (execution continuity). The design belongs to LOO-394;
this copy drives LOO-411's machine record. Jack Heart initially selected stacking
on the rename on October 7; its merged base is now integrated. LOO-393 and
LOO-395's intent is retained in the later slices.

## Full remote-work demo — later slices

```sh
lf machine add mini          # name the connection; offer installation if lf is missing
lf machine status            # reachable / needs sign-in / error; never prompts
lf --machine mini --task LOO-123 implement   # mini has never seen LOO-123; prints Session and reconnect command
# close the laptop, wait, open it
lf --machine mini session list --waiting
lf --machine mini session attach <id>        # same conversation, output so far, no repeated prompt
```

Then quit the Mac app mid-turn and reopen it: the same conversations are in the
same panes.

## Decided by Jack Heart (all 2026-10-07 unless dated)

- Remote agents keep running when the laptop closes, sleeps or drops (LOO-393).
- An existing Task is usable on the selected machine without recreating it.
- No daemon by preference: see whether plain ssh and detached processes suffice.
  Not opposed to one in principle.
- Follow herdr's lead on defining and connecting machines; do more credential
  forwarding than herdr.
- Credentials in scope: Claude, Codex, GitHub, Linear.
- Forward a credential wherever a forwarded copy keeps working. Where it cannot,
  either mint a separate grant on the laptop or sign in on the machine with the
  laptop's browser approving, whichever is more seamless or closes a hole. One
  approval per account per machine is acceptable; automate it.
- Forwarded credentials may live on the machine's disk where a normal login
  would put them.
- Security floor: "herdr should be used as a baseline for 'we can't be
  (meaningfully) less secure than this.'"
- Task store: Linear is "an easy default but not required"; explore the store
  explicitly. Requirement from Jack's earlier move away from Tasks-in-Git: one
  official copy, and a new Task visible everywhere without merging anything.
- Holder for interactive conversations: a transparent relay. "ok, relay it is."
  tmux is out ("made the terminal experience worse"); no holder at all is
  "probably also not desirable"; a multiplexer of our own is out because
  Loopflow will not compete on multiplexing. Jack expects to sit on a live
  Ghostty, on Superlogical's Rex, or on herdr over time.
- A resident process with a listener per Session is acceptable. Per Session
  means per agent conversation, not per Task or Wave.
- Restarting the Claude process on attach is acceptable; interrupting a turn
  "is not ideal but maybe not completely unsolvable. The important thing is
  just that it survives the laptop closing."
- Rotating credentials reach a machine by minting a fresh login on the laptop
  for that machine and forwarding it. "Yeah I'm confident in this direction."
  Loopflow logs in to its own private directory on the laptop, the person
  approves once in the browser, and the credential file is copied to the
  machine. No terminal on the machine; the laptop's own login is untouched.
- A launch that asks for an account the machine lacks starts adding that
  account to the machine: "if we know about account A on this machine, then we
  should start the process for adding A to the remote machine if it's not
  there." It never silently runs as a different account.
- Execution state does not cross machines. A Task's Workflow position,
  conversations and history stay on the machine where it runs. Running the
  same Task on a second machine starts its Workflow from the beginning there.
  That is accepted, with one requirement: "it should use the same remote
  branch, so if we run `implement` for example it would see the existing
  implementation."
- Tasks that already exist on two machines with different ids stay per-machine
  for now. "Probably going to require some iteration on this, but as long as
  the high-level approach is working, let's get it tested rather than block."
- Home becomes "machine", deep: what people read and type, and code and
  schema too. "Deep, but probably can be isolated into its own commit (I don't
  need to review it)."
- 2026-10-04/05, product Wave memory: no automatic restart or retry of a turn
  or Flow; the caller owns recovery; `-b` prints and blocks and the caller
  backgrounds; no hidden arguments; "do not restore that service".

## Baseline before the rename (v0.13.9 source)

PR 1 changes the Home names below to machine. The remote-work limitations remain.

- Home: random id minted with the store, plus a route. No name, no list, no
  remove. `lf home observe <id> <route>` records one unverified; routes must be
  `user@host`.
- `lf ssh`: `ssh … "bash -s"` with a preamble on stdin. No terminal. Remote `lf`
  found on a fixed PATH; repository hard-coded to `~/src/loopflow`; no version
  comparison. With a raw host it forwards credentials with no identity check.
- Credentials over ssh: origin broker lends access tokens over a forwarded
  socket and dies with ssh. `GH_TOKEN`, a Claude access token and the Linear
  token are exported as environment variables. Detached forms are rejected by a
  string check; detached launches strip forwarded credentials.
- Hangup: SIGHUP, SIGINT and SIGTERM run one handler that kills the provider and
  records the turn as interrupted. No path keeps a live turn across its
  launcher's death by design.
- Reattach: Codex only. A new client joins the live engine socket through a
  fenced relay. For Claude and OpenCode `session connect` starts a new client on
  the saved conversation.
- tmux: used to pre-start a primary conversation and for landing repair. Nothing
  attaches to it.
- Launch order is already reserve-before-spawn (Session row, prepared capture,
  driver claim, `reserved`, `spawn_requested`, spawn). No caller-supplied key.
- Tasks: `--task <ISSUE>` resolves locally only and fails with `Task "…" is not
  registered`. `lf task run <ISSUE>` falls through to Linear (LOO-334) but mints
  a second Task with new random ids, and plans the branch without fetching, so a
  pushed branch with no PR is recreated from main.
- Planning refresh has a 5-second deadline that is a hard error with no cache.
- Mac app: a pane runs `lf session connect <id>`; the app owns the terminal;
  quitting ends the clients. Pane layout is in memory only.

## PR 1: inherited rename

PR #1484 merged as `626789dcd` and is integrated. Jack Heart selected cache
discard and the `installation` scope; opaque IDs, selection/schedule keys, install
paths and historical payloads retain their bytes. Parent verification remains at
`06e88761d:scratch/work-on-another-machine-name.md`; earlier stack ancestry is at
`5840a8126:scratch/work-on-another-machine-name.md`. No installed migration is claimed.

## Machine record and later design

### This slice: machine record (LOO-411)

`lf machine add <ssh-target> [--label NAME] [--repo PATH]`, `list`, `status`,
`rename`, `remove`. Stored: label (default: the ssh host), target (anything
OpenSSH accepts, aliases included), the remote identity fetched during add, the
repository path. No credentials. Every command accepts the label. `remove`
never touches remote work. `status` never prompts. `lf --machine <label>` runs the whole command there and requires an added machine before any
credential crosses.

First connect probes the installed `lf` and reads its machine identity. Missing
lf produces an install offer only during interactive add, otherwise an install
command. Version differences produce an update instruction; a difference alone
does not reject a command. Identity or command failure stops it naturally.

Versions across machines (Jack Heart, 2026-10-07, comment
1958673f-baba-470e-b73a-1a544007dd86): no stable cross-version interface,
capability negotiation or support for older remotes. Status shows both versions;
connecting reports differences and names the remote update command. This
supersedes the earlier proposal. The October 7 PR review permits offering first installation during interactive add; an existing peer is never replaced.

Review decisions on PR 1 (Jack Heart, 2026-10-07): drop the Desktop cache
translation and discard old cache files; rename the install scope so "machine"
has one meaning; accept that machines reached by id upgrade together for now;
nothing outside the repository needs changing.

### 2. The Task on a machine that has never seen it

Proposal: Linear, GitHub and Git are already the official live copy. Derive.

- A missing `--task` selector falls into LOO-334's adoption path instead of
  failing.
- Fetch whenever a decision depends on what the remote has, starting with
  branch placement.
- Mint Task ids deterministically from the Linear issue id for new Tasks.
- The earlier proposal to send portable Task fields with `lf --machine` remains
  unimplemented. The accepted selector forwards command arguments only; a later
  Task-adoption plan must reconcile cold-machine discovery with that boundary.

The second machine works on the same remote branch (decided above). Placing a
Task fetches its branch and checks it out, so a skill run there sees the code
already written. Jack Heart, 2026-10-07: "fetch anytime it's necessary." Any
step that depends on what the remote has (placing a Task, choosing a base,
deciding whether a branch exists) fetches first instead of trusting a stale
local view. Today placement tests `origin/<branch>` without fetching, and
a pushed branch with no PR is recreated from main; that is the defect to
remove. Work that exists only on the first machine, unpushed, is not there:
say which branch or commit is missing and that it needs a push, and never
commit, push or reset on the person's behalf.

No residue store. A custom workspace slug, agent preference, an unpublished
stack parent and Workflow position stay on the machine that has them. The Git
ref namespace considered earlier is not needed.

Set aside from this approach: Linear attachments as the store (LOO-393's
draft). No compare-and-swap, one query per Task for catch-up, visible clutter
on the issue, and it needs a conflict-resolution command.

### 3. Credentials resident on the machine

One rule for every credential that rotates (decided above): mint a fresh login
on the laptop for that machine and forward it.

| Credential | Mechanism | Approvals |
|---|---|---|
| GitHub | Copy the `gh` token into the machine's `gh` config, file storage. Nothing rotates. | 0 |
| Claude | Fresh login into a private directory on the laptop; copy the credential file to the machine's account directory | 1 |
| Codex | The same, with its own private directory | 1 |
| Linear | A second grant minted on the laptop; store its tokens on the machine | 1 |

`lf account connect` already performs the first half: it runs the provider
login in a staging directory apart from the laptop's own login, with the
browser callback arriving on the laptop, and verifies the account. The new part
is installing the result on the machine instead of locally: write the file
atomically with owner-only permissions, verify identity there, register the
account.

Why this is safe where copying the existing login is not: each machine gets
its own refresh chain. Copying the laptop's login would put two machines on
one chain, and whichever refreshed first would sign the other out.

Fallbacks only if a full login cannot be forwarded for some provider: tunnel
the login's callback from the machine, relay a pasted code from the laptop's
prompt, or for Claude a `claude setup-token` (one year, inference-only, may
fail identity and usage checks).

Rests on one unverified fact, first on the list to test: a second login for
the same account does not sign out the first.

Conditions from the herdr floor: send only to an added machine with strict
host-key checking; name the accounts at add time; never on a command line or in
an exported environment; nothing in the background.

Mac targets: over ssh the login Keychain is locked. Keep everything file-backed
(account homes, `gh`, loopflow's token key). `store/token_crypto.rs` appears to
treat a failed Keychain read as "no key" and mint a new one, which would orphan
stored tokens; verify and fix before a Mac is a target.

When a launch names account A and the machine does not have A, `lf` starts
connecting A to that machine with the mechanism in the table, then runs. An
account the machine already has is kept, never overwritten. A different account
on the machine is left alone and is not used in A's place.

The existing lease stays for foreground commands on added machines until resident credentials replace it.

### 4. Work that outlives its launcher

Headless work needs no holder. `lf --machine <machine>` is the caller that
backgrounds: it starts the remote `lf -b …` in its own session with output to
its capture, prints the Session id and returns or follows. This is consistent
with "`-b` prints and blocks; the caller backgrounds". Requires part 3.

Launch request id: recorded on the origin before ssh, written on the target
inside the launch lock as a unique key on the Session. A retry returns the same
Session: launch if still prepared, attach if live, report unknown if spawn was
requested without evidence. Never a second agent.

Interactive conversations are held by a transparent relay (decided above).
A detached `lf` per Session owns the terminal device and passes bytes through
unchanged to whichever client is attached. Loopflow's Ghostty surface, or the
person's own terminal over ssh, does all emulation.

- `lf` allocates the terminal device and spawns the provider on it. It owns
  none today.
- It drains output with no client attached and writes it to the Session's
  capture.
- Window size travels from the attached client. One client sets the size.
- Reattach restores the modes the program set at start (alternate screen,
  mouse reporting, bracketed paste, keyboard protocol), then asks the program
  to repaint. Raw replay of captured output is not used: it contains queries a
  fresh terminal would answer.
- One Unix socket per Session, owned by the user, in the Session's runtime
  directory. No shared state between holders.
- The attach protocol carries a version; a holder outlives an `lf` upgrade.
- Kept small enough to delete. Where `lf` already runs inside herdr or Rex,
  that host holds the terminal and the relay is not started. No pluggable
  holder interface: one implementation, reshaped when a second host is real.
- Not needed for headless work, or for Codex, whose engine socket already
  reattaches.
- Known limits, accepted: a client on a different kind of terminal gets a
  degraded view; no feature reads the screen.

`session attach` over ssh: a second ssh call with a terminal and an argv-only
remote command, no secrets. Wire the existing unreachable remote branch of
`human_open_argv`.

Hangup on a viewer never signals the agent. Stop is an explicit command, which
`lf session` lacks today.

### 5. Mac app (was LOO-395)

Persist pane layout and which Session is in which pane, per Task. On launch,
each pane attaches to its Session's relay. Shell panes can use the same relay
(it holds any command) or reopen in the same directory; decide in review.

### 6. Seeing across machines

`lf machine status` shows a waiting count per reachable machine with the age of
the reading. Detail stays one call: `lf --machine mini monitor`. Reads are never
silently merged. Phone use is LOO-396.

## Open choices

No decision is needed for LOO-411. Later plans must resolve the retained draft's
Codex contradiction: the relay design exempts its existing engine socket, while
the deletion inventory lists it for replacement. Shell-pane restoration also
remains undecided. Neither changes the accepted per-Session holder or authorizes
later implementation.

## Sequence

Decided by Jack Heart, 2026-10-07: "Fine to break things up into multiple
PRs." Each one deletes what it replaces. Follow-ups are intent only; each gets
its own plan when started.

1. Rename Home to machine. PR #1484 is merged and integrated.
2. `This slice` (LOO-411). Machine record: `lf machine add`, `list`, `status`, `rename`, `remove`;
   ssh aliases; identity fetched at add; repository path. Deletes `lf machine
   observe`, the hard-coded repository path and the `user@host` rule.
3. A Task on a machine that has never seen it: one resolver, fetch whenever
   the remote decides, deterministic ids for new Tasks. Deletes the local-only
   `--task` path and random id minting for new Tasks.
4. Credentials on the machine: mint a fresh login on the laptop and forward it;
   connect a missing account at launch; file-backed storage on a Mac target.
   Deletes the raw-token fallback and, once resident credentials exist, the
   detached-form string check.
5. Headless work that outlives the connection: detached launch from `lf --machine`,
   the launch request id, an explicit stop, and watching recorded output.
   First end-to-end test on Jack's mini: start, close the laptop, come back.
6. The transparent relay and `session attach` over ssh. Deletes the tmux
   launcher with its hidden command, the setsid launcher's separate path if the
   relay subsumes it, and the unreachable remote branch of `human_open_argv`.
7. Mac app: saved pane arrangement, panes attach to their Session's relay.
8. Waiting counts per machine in `lf machine status`.

## Delete — do not maintain

The selector removes `MachineCommand::Ssh`, its origin-account/target boundary,
`normalize_ssh_args`, SSH-specific navigation/reordering and `reject_nested_ssh`.
Remote dispatch happens before local command parsing, inspection and placement.
The transport keeps credential and identity checks; no command-specific refusal
is added. The saved Machine repository replaces the old invocation override.
Process history retains the remote exit code. Local Work IDs are not forwarded.


Review additions replace the unshared bounded SSH arguments and undifferentiated
connection error on the existing probe/command path. No second transport or
installer implementation. Use the public installer; a private control directory
isolates Loopflow from personal SSH masters. OpenSSH expires idle connections
after 60 seconds; per-command account forwarding is cancelled on return. Explicit
agent forwarding uses a separate control namespace so a later ordinary probe
cannot inherit it. Headless, batch and JSON add report the manual install command.


Jack Heart, 2026-10-07: a redesign cleans up in the same diff. Where several
building blocks exist for one job and none quite works, delete them and keep
the code minimal; do not add another beside them.

Removed in this slice: `MachineCommand::Observe`, `Store::observe_machine`,
`SqliteStore::observe_machine`, `MachineRoute`/`MachineHost` and their parser
tests, `DEFAULT_REPO`, and the raw-host SSH fallback. Compression also removes
`run_with_env`, `ssh_connection_args`, the second SSH runtime, and per-machine
queries during listing. One row reader serves local, by-ID and list reads.
The review-addition compression removes `SshOutcome` and `ssh_args`: SSH returns
the existing command result directly, and forwarding setup/cancellation reuse
one route and connection argument vector. This avoids rebuilding transport state
during cleanup and retains remote exit codes, broker cleanup and diagnostics.
The selector compression removes the preamble's arbitrary `extra_env` bundle
and optional identity path: it takes the saved Machine and participant name
directly. Local normalization and remote selection share option-value scanning,
so command text and attached flag values keep the same boundaries.

Remaining targets belong to later slices. Each removes what it replaces;
Codex's disposition remains unresolved as noted above:
- Three ways to start something detached, none of which is a reattachable
  Session: `start_tmux_session` with the hidden `serve-conversation` command;
  `start_lf_session_inheriting` (setsid, used for landing repair); the Codex
  engine socket and relay.
- The remote branch of `human_open_argv`, which no caller reaches.
- Two Task resolvers: the local-only `--task` path and LOO-334's Linear
  adoption path.
- The lease broker's detached-form string check
  (`reject_detached_account_forwarding`) and the ambient fallback that exports
  the laptop's raw provider tokens over ssh.
- Random Task id minting for new Tasks.
- `/detach` remains with the later relay slice.

The inherited rename already removed `capture_home_command`, its unused capture
subprocess and error type, and the cache migration's duplicate serialization.
Installer subprocesses use `install` so retained binaries remain callable across
the rename. The ignored `home:` GOAL key retains its rejection fixture.

## Forbidden outcomes

- A shared Loopflow resident process, or anything that restarts or retries a turn or Flow.
- tmux as the holder, or a terminal emulator of our own inside the holder.
- Hidden arguments.
- A secret on a command line, or credentials sent to a host that was not added.
- Duplicate Tasks for one issue on one machine; existing per-machine IDs remain
  as Jack Heart accepted above.
- Task records as files in the working tree.
- A memory-only credential owner that needs the laptop after every restart.

## Constraints

- Machine-local paths, liveness and execution authority never travel. A received
  record is never a worker claim.
- One migration draft for LOO-411, dependent on the inherited rename draft.
  Preservation fixtures start from released SQL and apply both finished drafts;
  no intermediate draft schema is a supported upgrade frontier.
- Wire types get no defaults; fixtures updated in Rust and Swift.

## Full remote-work acceptance — later slices

- Two isolated machines over a real ssh transport: add, status, `--task` on a
  machine that lacks the Task, repeat without a duplicate Task or checkout.
- Disconnect before reservation, after reservation, after spawn, and mid-turn:
  one Session, work continues, reattach shows its output.
- No secret value in argv, logs, records or artifacts.
- Quitting and reopening Desktop restores the same conversations in the same
  panes; detaching leaves the agent running and explicit stop ends it.
- Deferred to hand verification on Jack's mini (needs real accounts):
  1. A second Claude login and a second Codex login for the same account, made
     in a private directory: the laptop's own login still works after both
     have refreshed.
  2. Two Linear grants refreshed a day apart stay independent.
  3. The mini over ssh after a reboot with nobody logged in.
  4. Codex device code over ssh; laptop still signed in after eight days.
  5. `gh` accepts a copied token and both machines keep working.

## Evidence

The superseded pre-selector walkthrough is preserved at
`6886ec8db5cfac61bf74c6e2704aeb78270ad5bf:scratch/pr-review.html`.
Its unshared-SSH, missing-install-offer and stacked-base findings were resolved
as described below; it no longer describes the current review surface.

- LOO-393's one commit (`d19a24a19`) is retained in Git as
  `08191d19269af96e15d79918689a04b4fa44dc30:scratch/loo-393-checkpoint.patch`. `TaskRecord`, `PrRecord` and `from_task`
  in its `task_record.rs` fit any store; the revision and attachment code fits
  only the option set aside. Its `wave/infrastructure/MEMORY.md` hunk is
  unrelated memory curation and is not carried.
- Comparison guides: https://claude.ai/artifact/1xpMiizmX3od7pYpqBTdcQ and
  https://claude.ai/artifact/MsYW6XqaKpJoUzcx5oRxfS

Parent rename evidence remains at
`06e88761d:scratch/work-on-another-machine-name.md`. PR #1484 owns its gate,
installation proof and accepted cache/install-scope decisions. This slice does
not claim installed migration or later remote-work acceptance.

LOO-411 implementation choices (2026-10-07): extend the existing Machine row
with optional label and repository path. Removal clears those connection fields,
preserving identity, route history and placements. Legacy observed routes require
an explicit add before forwarding credentials. Probe uses existing `lf --version`
and `lf machine id`; no provider login participates in it.
An omitted repository uses the local checkout's home-relative path; outside a
checkout, `--repo` is required. Absolute remote paths and `~/` paths are accepted.
Version differences are reported with an update command, without negotiation or
a version gate. Failed identity reads retain any reported version. Installation
is offered only for a missing executable; existing peers are never replaced.
The deletion inventory above is complete for this slice. Exact machine checks
and credential transport via stdin remain. Focused fixtures cover label lifecycle,
nonprompting probe failures, quoting, version reporting and preservation from
released SQL in disposable stores.

Review finding: removal must release the active destination as well as the label,
so a replacement machine can be added there without deleting the old identity.
The route's uniqueness index covers the local identity and named connections;
unnamed history remains intact. An unsupported peer still contributes its version
when the identity command fails. Machine commands and the command reference are
implemented. PR #1489 is the review surface for these additions.
Sync preserved the machine changes and adopted upstream Process naming; focused
checks cover the combined tree.

Review additions preserve the existing identity-before-credential path. The
simulated installer proves the interactive offer, refusal, default acceptance,
reprobe and registration, and the no-install paths for existing/broken lf,
status, batch and JSON. It proves no downloaded installer or migration.
A disposable loopback sshd and generated test keys exercised add, status and two
ssh commands through one TCP connection while a separately configured personal
master remained usable. Its remote lf was a stub; this proves transport reuse,
not account forwarding or configured mini continuity. OpenSSH semantics come from
[ssh_config](https://man.openbsd.org/ssh_config) and [ssh](https://man.openbsd.org/ssh).

Simulated review findings fixed: distinguish a genuinely absent executable from
an existing lf exiting 127; cancel each account-forwarding route after its command
because a persistent master otherwise retains it. Explicit agent forwarding has
its own socket namespace. No schema or later-slice changes.

Earlier migration, command-discovery and Swift fixture results, including the
two open-output-handle observations, remain at
`e7cd8050ee3882a71a2374c693d0ef6543b2219c:scratch/work-on-another-machine-name.md`.

Prior loopback SSH command/evidence and the exit-classifier output-handle warning
remain at `5840a8126:scratch/work-on-another-machine-name.md`.

Review findings fixed: the custom root help initially hid the selector; it now
shows placement and forwarding options within its existing 25-line limit. A
builtin remote example used ambiguous `status`; it now names `wave status`.
Origin-side account resolution would reject a target-only account; explicit
account flags now reach the target unchanged. The public CLI fixture runs the
compiled remote CLI against a separate registry and proves `machine add` changes
that registry only. Simulated transports do not establish configured SSH/account
continuity; the earlier real SSH sharing proof still covers unchanged transport.

Checks: `git diff --check` and `lf context --skill realign` pass; unchanged code reuses `b347fe4d0`'s recorded fmt, all-target Clippy and 42 focused nextest passes (LF_/LOOPFLOW_ cleared, LF_BIN pinned), including the open-output-handle warning in `preamble_omits_absent_credentials`; exact commands and prior broader evidence remain at `b347fe4d0:scratch/work-on-another-machine-name.md`. Affected gate, leak investigation and disposable-account published-installer proof remain with gate/CI.
