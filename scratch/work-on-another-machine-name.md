# Work on another machine

Status: PR #1484 is published. Jack Heart requested only two review changes on
2026-10-07 (comment 55a0096b-1a36-4690-bb2e-a1a3a1763989): discard old Desktop
caches and rename the installation scope. Both are implemented and focused checks pass;
republication remains. Stop after republication; no landing or later slice
is authorized. The full affected gate and isolated installation proof remain
with gate/CI.

Do not lift code, tests, help text or config from herdr or cmux (Jack Heart,
2026-10-07). Their behaviour is described here from docs and reading; write
loopflow's version from scratch.

## What to build

Someone with a second machine names it once, starts a Task there from the
laptop, closes the laptop, and returns to the same work from the laptop, the
Mac app or a phone.

## Placement

Infrastructure Wave (execution continuity). LOO-394. LOO-393 and LOO-395 are
merged into it.

## The demo

```sh
lf machine add mini          # ssh works, lf found or installed, identity recorded, accounts connected
lf machine status            # reachable / needs sign-in / error; never prompts
lf ssh mini --task LOO-123 implement   # mini has never seen LOO-123; prints Session and reconnect command
# close the laptop, wait, open it
lf ssh mini session list --waiting
lf ssh mini session attach <id>        # same conversation, output so far, no repeated prompt
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

## PR 1: rename — review changes implemented

Rename slice: commands, help, errors, docs, builtin skills, Rust/Swift types and
DTO fixtures use machine. One migration renames the identity table and placement
columns without rewriting IDs, routes, timestamps or historical payloads.

- `LF_HOME`, provider homes and account homes retain their filesystem meanings.
- Opaque `home_…` IDs stay unchanged, including scheduled-job references.
- Saved cron JSON accepts its released `home_id` key and writes `machine_id`.
  The installed `LoopflowHomeId` plist key and Desktop selection's `homeId` key
  remain stable; changing them would drop existing schedules or selections.
  Desktop discards version-1 cache files and reloads current data. Jack Heart
  explicitly accepted losing those saved views instead of translating them.
- The retired landing discriminator and JSON retain their historical spelling
  and fence. Released SQL and historical benchmark captures are immutable.
- Installation code is `installation.rs`; `InstallationCommand` owns install
  and skill exports. The existing `lf install` shorthand survives; the hidden
  skill exporter uses its full `lf installation sync-skills` path.
  Keep `~/.lf-machine/install` and `~/.lf/promotion.lock`: released entry gates,
  receipts and jobs pin them. No installed relocation proof exists, so neither
  path moves. Jack Heart explicitly authorized this expanded rename.
- A machine is one OS user and data directory, documented in the glossary.

PR 1 acceptance: CLI/help/docs and Rust/Swift DTOs agree on machine; the released
schema reaches the single draft with IDs, routes, timestamps and foreign keys
intact; cron ownership and retained installer recovery survive the rename.
Current-version Desktop caches retain selections; old caches are discarded. Existing focused checks cover these source boundaries.
The affected gate and isolated installer proof remain with gate/CI. Publication
does not establish installed migration or any remote-work acceptance below.

## Later design — not started

### 1. Machine record

`lf machine add <ssh-target> [--label NAME] [--repo PATH]`, `list`, `status`,
`rename`, `remove`. Stored: label (default: the ssh host), target (anything
OpenSSH accepts, aliases included), the remote identity fetched during add, the
repository path. No credentials. Every command accepts the label. `remove`
never touches remote work. `status` never prompts. `lf ssh <label>` stays the
one way to run a command there and requires an added machine before any
credential crosses.

First connect: probe, find `lf`, check capability, offer install (default yes).
Replacing a running remote `lf` asks first and says what would stop.

Versions across machines (proposal; Jack Heart asked on 2026-10-07 whether to
keep `lf` versions in sync across machines or make mixed versions compatible).
Loopflow ships daily and the style guide rules out compatibility layers, so
skew will be common and supporting arbitrary old versions is the expensive
answer. Proposed instead: detect skew and make it one step to fix.
- Connecting asks the remote what it can do through one stable public command,
  not through whichever command the current release happens to use. PR 1 showed
  the cost: the identity check runs `lf machine id`, so a remote one release
  behind fails with an unhelpful message.
- `lf machine status` shows each machine's version beside the local one.
- When the remote is behind, say so and offer to update it (default yes).
  Updating never stops work running there without an explicit yes.
- Each machine's own scheduled update keeps running; this covers the window
  between them.

Jack Heart accepted that machines reached by id upgrade together for now;
nothing outside the repository needs changing.

### 2. The Task on a machine that has never seen it

Proposal: Linear, GitHub and Git are already the official live copy. Derive.

- A missing `--task` selector falls into LOO-334's adoption path instead of
  failing.
- Fetch whenever a decision depends on what the remote has, starting with
  branch placement.
- Mint Task ids deterministically from the Linear issue id for new Tasks.
- `lf ssh` sends the issue identifier and the Task's portable fields with the
  request, so the first run on a cold machine needs no Linear round trip.

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

Today's lease stays for foreground commands on machines not yet connected.

### 4. Work that outlives its launcher

Headless work needs no holder. `lf ssh <machine>` is the caller that
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
the reading. Detail stays one call: `lf ssh mini monitor`. Reads are never
silently merged. Phone use is LOO-396.

## Open choices

No decision is needed for PR 1. Later plans must resolve the retained draft's
Codex contradiction: the relay design exempts its existing engine socket, while
the deletion inventory lists it for replacement. Shell-pane restoration also
remains undecided. Neither changes the accepted per-Session holder or authorizes
later implementation.

## Sequence

Decided by Jack Heart, 2026-10-07: "Fine to break things up into multiple
PRs." Each one deletes what it replaces. Follow-ups are intent only; each gets
its own plan when started.

1. Rename Home to machine. `This slice`: PR #1484 published; two review changes
   implemented and focused checks pass; republication remains. Old display caches are
   discarded by explicit decision. Gate and isolated installer proof remain.
2. Machine record: `lf machine add`, `list`, `status`, `rename`, `remove`;
   ssh aliases; identity fetched at add; repository path. Deletes `lf machine
   observe`, the hard-coded repository path and the `user@host` rule.
3. A Task on a machine that has never seen it: one resolver, fetch whenever
   the remote decides, deterministic ids for new Tasks. Deletes the local-only
   `--task` path and random id minting for new Tasks.
4. Credentials on the machine: mint a fresh login on the laptop and forward it;
   connect a missing account at launch; file-backed storage on a Mac target.
   Deletes the raw-token fallback and, once resident credentials exist, the
   detached-form string check.
5. Headless work that outlives the connection: detached launch from `lf ssh`,
   the launch request id, an explicit stop, and watching recorded output.
   First end-to-end test on Jack's mini: start, close the laptop, come back.
6. The transparent relay and `session attach` over ssh. Deletes the tmux
   launcher with its hidden command, the setsid launcher's separate path if the
   relay subsumes it, and the unreachable remote branch of `human_open_argv`.
7. Mac app: saved pane arrangement, panes attach to their Session's relay.
8. Waiting counts per machine in `lf machine status`.

## Delete — do not maintain

Jack Heart, 2026-10-07: a redesign cleans up in the same diff. Where several
building blocks exist for one job and none quite works, delete them and keep
the code minimal; do not add another beside them.

Earlier attempts found in source. Each later slice removes what it replaces;
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
- `lf machine observe` (renamed in PR 1); the hard-coded `~/src/loopflow`; the `user@host` rule.
- Random Task id minting for new Tasks.
- Rename slice removes `capture_home_command`, its unused capture subprocess and
  error type. Review removes `renameMachineFields`, the version-1 cache translation
  and its tests; the existing discard and current-cache tests cover the survivor. Installer
  subprocesses use `install` without a command-group prefix so retained binaries
  remain callable across the rename. The `home:` GOAL key is already ignored by
  config parsing; its rejection fixture stays. `/detach` remains with the later
  relay slice.

## Forbidden outcomes

- A shared resident process, or anything that restarts or retries a turn or Flow.
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
- One schema migration, written against the last released schema.
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

- LOO-393's one commit (`d19a24a19`) is retained in Git as
  `08191d19269af96e15d79918689a04b4fa44dc30:scratch/loo-393-checkpoint.patch`. `TaskRecord`, `PrRecord` and `from_task`
  in its `task_record.rs` fit any store; the revision and attachment code fits
  only the option set aside. Its `wave/infrastructure/MEMORY.md` hunk is
  unrelated memory curation and is not carried.
- Comparison guides: https://claude.ai/artifact/1xpMiizmX3od7pYpqBTdcQ and
  https://claude.ai/artifact/MsYW6XqaKpJoUzcx5oRxfS

Review: the publication range retains identities and stored paths. The cache
translation is deleted; current-version saved data and incompatible-file removal
keep their existing coverage. Install and sync-skills move to `installation`,
while machine diagnosis still inspects the machine's storage and execution.
Retained binaries are called through the existing `install` shorthand. The
promotion lock keeps the same inode path, preventing split exclusion across
versions. No schema or installation-path migration was added.

The prior review walkthrough is historical at
`56a2677c4:scratch/pr-review.html`. It led to Jack Heart's two accepted changes.
The command discovery test exposed a pre-existing half-renamed fixture: its
synthetic tree still named `home` while expecting `machine`; the fixture now
matches the expected command. The scheduled installer also still emitted
`home install`; its generated job now uses `install`. The schedule fixture
checks retained cadence and repeatability; no live job was rewritten. Hidden
commands require their full owner path; they have no transitive shorthand.
Earlier migration/DTO/cron/Python evidence remains
at `d7a345896:scratch/work-on-another-machine-name.md`; retained installer repairs
are at `327e6e11e`. Release's child memory still requires preserving original job
ownership and activation; no installed or remote acceptance is inferred here.

Checks: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` and `git diff --check` pass; focused `cargo nextest run -p loopflow --lib --bin lf --test cli_discovery --test documented_commands --test global_commands -E 'test(installation::tests::) | test(lf::commands::install::) | test(install_exposes_refresh) | binary(cli_discovery) | binary(documented_commands) | test(scheduled_install) | test(schedule_rejects)'` covers 62 checks (61 passed together, corrected help/discovery and documentation rerun: 4/4); `scripts/test_desktop.sh -Xswiftc -gnone --filter WorkCacheTests` passes (14). Full affected gate and isolated installer proof remain with gate/CI; no installed upgrade or remote continuity is claimed.
