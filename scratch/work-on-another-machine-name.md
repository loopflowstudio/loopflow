# Work on another machine

Status: draft for review, 2026-10-07. Nothing here is accepted except the items
under "Decided by Jack Heart". Everything else is proposal or open choice.

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
- Home becomes "machine", deep: what people read and type, and code and
  schema too. "Deep, but probably can be isolated into its own commit (I don't
  need to review it)."
- 2026-10-04/05, product Wave memory: no automatic restart or retry of a turn
  or Flow; the caller owns recovery; `-b` prints and blocks and the caller
  backgrounds; no hidden arguments; "do not restore that service".

## Current system (v0.13.9, read from source)

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

## Design

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

Rename, in its own commit with no behaviour change (decided above): commands,
help, errors, docs, builtin skills, Rust and Swift identifiers, wire fields and
fixtures, tables and columns. Points the commit must settle:
- `LF_HOME` names the data directory, a different thing; it keeps its name.
- "Provider home" and "account home" are also different things and stay.
- "machine" already names the install scope in code (`machine_install.rs`, the
  machine promotion lock). Rename or absorb that use so one word has one
  meaning.
- A Home is per data directory and OS user, so two users on one box are two
  machines in this vocabulary. Say so in the glossary.
- The `home_` id prefix is stored in every database and scheduled job. Either
  migrate stored ids or keep the stored value and rename everything around it;
  the second avoids rewriting identifiers already written to launchd plists.
- Rough size: `home_id` about 240 and `HomeId` about 83 in Rust, about 167
  `homeId` in Swift, about 72 SQL occurrences, about 158 doc mentions.

### 2. The Task on a machine that has never seen it

Proposal: Linear, GitHub and Git are already the official live copy. Derive.

- A missing `--task` selector falls into LOO-334's adoption path instead of
  failing.
- Fetch the branch before planning placement.
- Mint Task ids deterministically from the Linear issue id for new Tasks.
- `lf ssh` sends the issue identifier and the Task's portable fields with the
  request, so the first run on a cold machine needs no Linear round trip.

The second machine works on the same remote branch (decided above). Placing a
Task fetches its branch and checks it out, so a skill run there sees the code
already written. Today placement tests `origin/<branch>` without fetching, and
a pushed branch with no PR is recreated from main; that is the defect to
remove. Work that exists only on the first machine, unpushed, is not there:
say which branch or commit is missing and that it needs a push, and never
commit, push or reset on the person's behalf.

No residue store. A custom workspace slug, agent preference, an unpublished
stack parent and Workflow position stay on the machine that has them. The Git
ref namespace considered earlier is not needed.

Set aside pending Jack's decision: Linear attachments as the store (LOO-393's
draft). No compare-and-swap, one query per Task for catch-up, visible clutter
on the issue, and it needs a conflict-resolution command.

### 3. Credentials resident on the machine

| Credential | Mechanism | Approvals | Evidence |
|---|---|---|---|
| GitHub | Copy the `gh` token into the machine's `gh` config, file storage | 0 | No refresh token; nothing rotates |
| Codex | Device-code login on the machine, approved in the laptop's browser | 1 code | OpenAI docs: do not share `auth.json` across machines; a copy lasts about 8 days |
| Claude | `claude setup-token` minted on the laptop, or `claude auth login` on the machine with a pasted code | 1 | Refresh is single-use; see open choice 2 |
| Linear | A second grant minted on the laptop | 1 | Each refresh issues a new refresh token; 30-minute grace |

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

1. Claude on a machine. Jack Heart asked on 2026-10-07 whether a normal login
   can be had without typing into the machine's terminal. Three ways, in order
   of preference:
   - Mint a fresh login on the laptop and forward it. `lf account connect`
     already runs a provider login in a private staging directory, apart from
     the laptop's own login, with the browser callback arriving on the laptop.
     Do that once per machine and move the resulting credential file into the
     machine's account directory. One browser approval, no tunnel, no terminal
     on the machine, full scope, its own refresh chain. The same move works for
     Codex and matches the second grant already planned for Linear.
   - Run the login on the machine and tunnel its callback. Claude's callback
     port is not fixed, so `lf` would read it from the authorization address
     and add a forward for it. No paste; more moving parts.
   - Run the login on the machine and relay the pasted code from the laptop's
     own prompt. One paste, at the laptop.
   `claude setup-token` remains the fallback only if a full login cannot be
   forwarded: it is inference-only and may fail loopflow's identity and usage
   checks.
   All of these rest on one unverified fact: a second login for the same
   account does not sign out the first.
2. Tasks that already exist on two machines with different random ids: backfill
   rule, or leave per-machine?
3. Shape: one PR, or a keystone (parts 1 and 2) with parts 3 to 6 as follow-ups?

## Delete — do not maintain

Jack Heart, 2026-10-07: a redesign cleans up in the same diff. Where several
building blocks exist for one job and none quite works, delete them and keep
the code minimal; do not add another beside them.

Earlier attempts at this job found in source. The relay and the detached
headless launch replace them, so they go in the same change:
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
- `lf home observe`; the hard-coded `~/src/loopflow`; the `user@host` rule.
- Random Task id minting for new Tasks.
- `capture_home_command` (no callers); the `home:` GOAL key (no reader); the
  `/detach` tmux leftover.

## Forbidden outcomes

- A shared resident process, or anything that restarts or retries a turn or Flow.
- tmux as the holder, or a terminal emulator of our own inside the holder.
- Hidden arguments.
- A secret on a command line, or credentials sent to a host that was not added.
- Two Tasks for one issue.
- Task records as files in the working tree.
- A memory-only credential owner that needs the laptop after every restart.

## Constraints

- Machine-local paths, liveness and execution authority never travel. A received
  record is never a worker claim.
- One schema migration, written against the last released schema.
- Wire types get no defaults; fixtures updated in Rust and Swift.

## Done when

- Two isolated Homes over a real ssh transport: add, status, `--task` on a
  machine that lacks the Task, repeat without a duplicate Task or checkout.
- Disconnect before reservation, after reservation, after spawn, and mid-turn:
  one Session, work continues, reattach shows its output.
- No secret value in argv, logs, records or artifacts.
- Deferred to hand verification on Jack's mini (needs real accounts):
  1. Claude `setup-token` in a fresh config directory: interactive use, identity
     check, usage polling.
  2. Two Linear grants refreshed a day apart stay independent.
  3. The mini over ssh after a reboot with nobody logged in.
  4. Codex device code over ssh; laptop still signed in after eight days.
  5. `gh` accepts a copied token and both machines keep working.

## Evidence

- LOO-393's one commit (`d19a24a19`) is saved as
  `scratch/loo-393-checkpoint.patch`. `TaskRecord`, `PrRecord` and `from_task`
  in its `task_record.rs` fit any store; the revision and attachment code fits
  only the option set aside. Its `wave/infrastructure/MEMORY.md` hunk is
  unrelated memory curation and is not carried.
- Comparison guides: https://claude.ai/artifact/1xpMiizmX3od7pYpqBTdcQ and
  https://claude.ai/artifact/MsYW6XqaKpJoUzcx5oRxfS

Checks: source and docs inspection on 2026-10-07; no builds or tests run.
