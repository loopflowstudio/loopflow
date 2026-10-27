# LOO-628: `lf resume` moves a conversation to another account or Machine

Status: design reviewed with Jack Heart 2026-10-26 (concept review). Slices 1, 2 and 3,
the pre-handoff check of M, the `lf resume` help and `lf --machine M resume --task X` running the Task move built (2026-10-26/27); the demo remains. Product direction is Jack Heart's
(LOO-610 conversation, 2026-10-26) and is binding; mechanisms below are chosen
here and reversible.

## Problem

`lf resume` reopens a conversation only where it already lives: same login,
same Machine. Moving it to a login with capacity, or to the mini or a rented
machine, means committing, pushing, copying history and restarting by hand.
Beneficiary: anyone whose login ran out mid-Task, or who wants the conversation
on a bigger or always-on machine.

## Demo

```bash
# laptop, Task LOO-12 mid-conversation in an isolated Claude home on `personal`
lf --account claude=work@ resume --task LOO-12
#   → the same Claude conversation (same session id, full scrollback) opens on `work`
lf session list --task LOO-12          # one Session, account `work`

# Jack's spelling (2026-10-26): flags after `resume`, a bare login email, `-a` picks the provider
lf resume --account jackstah@gmail.com -a claude
#   → same move, onto the Claude login jackstah@gmail.com; no `claude=` prefix

# laptop, uncommitted edits in the Task worktree: move the Task
lf task move LOO-12 --machine mini
#   Committed 3 files on jack/loo-12-… and pushed
#   LOO-12 now runs on mini (Workflow at `explore`); its Session moved with it
#   → `lf resume --task LOO-12` from anywhere opens Claude on mini, mid-conversation
# the old laptop terminal prints: "This conversation moved to mini: lf resume --task LOO-12"

# any conversation, Task or not: move only the conversation
lf --machine mini resume <session>
#   → same Session on mini in its own worktree of the branch; a Task stays where it was
```

Mid-merge worktree: either move exits 1 with
"LOO-12's worktree is mid-merge; finish or abort the merge, then move it"
and changes nothing.

## Risk findings (2026-10-26)

- **Isolated account moves silently start an empty conversation today.**
  `resolve_selected_provider_account` sets `resume_requested_session` only when
  the conversation's pinned account equals the chosen one; `harness/claude.rs`
  (start) and `harness/codex.rs` (start) then drop the session id. So
  `lf --account claude=work@ resume` on an isolated conversation launches a fresh,
  history-less Claude. This is the core defect this Task fixes.
- **Shared-home account moves already resume natively.** A shared conversation
  lives in the native home; naming another account signs that home in as it and
  resumes the same id (`shared_conversation_resumes_in_the_native_home_after_a_switch`).
  Needs an end-to-end test, not new mechanism.
- **Both providers resume a copied native transcript from a fresh home.** Probe
  with no credentials: Claude 2.x with `CLAUDE_CONFIG_DIR` holding only
  `projects/<slug>/<id>.jsonl` got past lookup to "Not logged in"; a missing id
  says "No conversation found". Codex 0.164 with `CODEX_HOME` holding only
  `sessions/Y/M/D/rollout-…-<id>.jsonl` (no `state_5.sqlite`) reached the
  Responses API (401); a missing id says "no rollout found". So same-Machine
  account moves can always resume natively when the transcript is readable, and
  the log path needs that same transcript. The log fallback therefore only
  matters if a provider *rejects* copied history at runtime.
- **Unverified: Codex encrypted reasoning across ChatGPT accounts.** Codex
  rollouts carry `encrypted_content` reasoning; OpenAI may reject it under a
  different account. Shared-home switching already assumes it works. Demo checks
  it with two real Codex logins; if rejected, Codex account moves take the log
  path: `config::resume_choice` (today it picks `Resume::Import` only for a
  provider change) also returns `Import` for a Codex account change, so the
  move reuses `conversation_log` unchanged.
- **Remote checkout already exists.** `TaskSource::resolve` refuses dirty or
  unpushed worktrees; the destination's `prepare_task` checks out
  `source.branch` at `source.commit`. The move adds the commit+push before it.
- **Ownership reaches the target without waiting for Linear.** `handoff_task`
  writes an entry whose comment marker `ingest_marker_in` already reads on other
  Machines. Carrying that marker in the move lets the target ingest it
  immediately instead of waiting on planning sync.
- **`--machine` was pure transport.** Before this Task, `lf --machine mini resume --task X`
  forwarded to mini, where the Task still routed to the laptop and failed.
  Nothing to keep compatible; it now runs the Task move first.
- **`lf session list` shows no account.** `SessionRecord` has `provider` and
  `workspace.machine_id` but no account; the account acceptance needs one field.
- LOO-618 (takeover by default) has not landed: `--takeover` is still opt-in. A
  move stops the old client itself; it is an explicit request to move.

## Chosen approach

### Same Machine, other account: adopt the native transcript

1. `provider_account`: when a requested conversation resolves to a route whose
   home does not hold its transcript (isolated route on another account, or an
   isolated↔shared change), copy the transcript into the route's home before
   launch: Claude `projects/<slug>/<id>.jsonl` plus the sibling `<id>/`
   directory if present; Codex the rollout file at the same `sessions/…`
   relative path. Locate the source with `transcript::locate` (it already
   resolves account homes). Never overwrite a newer file at the destination.
2. Pin the conversation to the new account and home (`pin_provider_session_route`
   is an upsert). The route always resumes the requested conversation.
3. Delete `resume_requested_session` and both harness branches that drop the
   session id. A conversation whose transcript cannot be found fails with
   `transcript::Gap`'s message instead of starting empty.
4. The move stops the old agent (`stop_session_agent`) before relaunch, as
   `import` already does. LOO-618 (default takeover) is unstarted as of
   2026-10-26, so the move does not depend on it; when it lands, its takeover
   replaces this explicit stop. The old client prints
   `This conversation moved to account {label}` when its agent exits and the
   Session's pinned account changed (same hook as the Machine message below).
5. `SessionRecord` gains `account: Option<ProviderAccountId>` (the current
   input's pinned account); `lf session list` prints it. DTO fixtures
   (`session.json`, `sessions.json`, `session_page.json`) and Rust + Swift
   fixture tests follow. Desktop shows nothing new.

6. `lf resume --account <email> -a claude` is the move's everyday spelling
   (Jack Heart, 2026-10-26). It already parses: `reorder_args` lifts
   `--account` to the global position, and a selector with no `provider=`
   matches across providers. The `-a` harness narrows it: with
   `-a claude`, a bare selector means `claude=<email>`, so a Codex login that
   shares the email never wins and never makes the selector ambiguous. Test:
   catalog with `claude` and `codex` accounts on the same email,
   `--account <email> -a claude` resolves exactly the Claude one, and the move
   resumes the same session id on it.

Another provider (`lf -a codex resume`) keeps the existing `Resume::Import`
log path; `--account` composes with it unchanged.

### Another Machine

See "Concept review" below: a Session move as the sub-API, a Task move on
top. The relay is unchanged from kickoff: `lf --machine M …` resolves the
source S; `send` runs on S and `receive` on M, each locally when that side is
this Machine, otherwise via `ssh::request` (gains an optional stdin body), so
laptop→mini, rental→laptop and rental→mini are the same code and the laptop
never needs to accept SSH. `conversation_log::continuation` is shared with
`import` when the log carries history. The old client, on agent exit, prints
`This conversation moved to {label}: lf resume {session}` (or
`lf resume --task {identifier}` after a Task move).

## Concept review (2026-10-26)

### Decided (Jack Heart, 2026-10-26, LOO-628 conversation)

- **Two moves, layered.** A *Session move* carries one conversation (branch,
  history, identity) to another Machine, for any Session. A *Task move*
  carries a Task (ownership, Workflow, branch, its Session) and uses the
  Session move as its sub-API. Both are built in this Task (Jack: "I want to
  do that here").
- **A Task's Session moved on its own moves only the conversation** (option
  a). The Task stays owned where it was. Jack: "assume people only do this if
  they know what they're doing … people should use the task move API". No
  refusal, no warning machinery.
- `lf resume --account <email> -a claude` works and `-a claude` implies the
  Claude login (step 6). Account moves are open to every Session.

### The two commands

```bash
lf --machine mini resume <session>       # Session move: this conversation only
lf task move LOO-12 --machine mini       # Task move: ownership + Workflow + branch + Session
```

`lf task move` already changes Wave membership "while preserving its recorded
work"; `--machine` is the same verb on the other axis. It replaces
`lf machine use --task`, which is deleted (Delete list).

### Session move (sub-API)

- `send` on S: refuse a mid-operation worktree naming the state; commit and
  push the cwd's branch; read history; stop the agent; complete the Session
  here with the target Machine recorded; emit `Move`.
- `receive` on M: check out {repository, branch, commit} in a worktree of the
  same repository (existing `lf wt` path, cloning when absent); admit the
  Session with the same id, name and title source; write the native
  transcript into the target route's home (Claude: under the target cwd's
  project slug; Codex: same `sessions/…` path) and pin it, so the same
  AgentSession resumes. If M's provider refuses the transcript at launch, fall
  back to `conversation.md` + continuation input from the same bytes
  (`conversation_log`).
- `lf resume <id>` / `lf session connect <id>` on S follows the recorded
  target. For a Task Session moved alone, `lf resume --task X` on S finds
  the completed primary and follows it the same way.
- A cwd outside a repository or on a detached HEAD refuses to move.

### Task move

`lf task move X --machine M`, in order, stopping at the first failure with
nothing handed off:

1. Refuse a mid-operation worktree, naming the state (before anything stops).
2. Stop any live Graph process for X and its step Processes (Jack Heart,
   2026-10-26: "stop it"). The Task stays on its route; a stopped Graph is
   history. The checkpoint below commits whatever the step had written, so
   only the step's in-flight turn is lost. On M, `lf task run X` runs the
   slice again; the carried `edge` says which.
3. Session move's `send` for the Task worktree and its primary Session (if
   any). The commit includes `scratch/`, so the working design travels.
4. `handoff_task(M)` (existing ownership log entry and marker).
5. `Move` gains `task`: {`ownership_marker`, `workflow`: {captured graph,
   node, edge, moves[]}}. Workflow position and history are local rows
   (`task_workflows`, `task_workflow_moves`); planning sync carries only the
   selection, so the move carries the rest.
6. `receive` on M: `ingest_marker_in`, `prepare_task` with the carried
   branch (existing remote checkout) instead of the Session move's plain
   worktree, write the Workflow position, append the moves (process ids
   nulled, since those processes live on S), then the Session move's admit.

Past Graph processes and step Processes stay on S as history; the target
starts its own. `lf task status X` on S routes to M through ownership.

`Move` (one wire DTO, no defaults): `from_machine_id`, `session`
{id, title, title_source, provider, model, primary} or null for a Task with no
Session, `checkout` {repository_id, origin, branch, commit} or null for a Task
never checked out, `history` {agent_session_id, files, log}, `task`
{task_id, identifier, ownership_marker, workflow {graph, node, edge, moves}}
or null. Fixture `tests/fixtures/dto/move.json`.

Decisions made while building slice 3 (2026-10-27, reversible):
- The initiator orchestrates: `send` on the Task's owner (locally or
  `lf session send --task X --to M` over SSH), `receive` on M (locally or
  `lf session receive` over SSH), so either end may be the caller. The
  initiator also ingests the handoff marker, so it routes to M at once.
- A Task on a route arrives on the same edge, carried by the receiving
  Process (`task_workflows` requires a carrier for an edge). That Process
  ends with the move, so the route reads as stopped and `lf task run` there
  runs its slice again. Moves already on M are not appended twice.
- Stopping a Graph: interrupt the Task (so its Task run starts nothing
  again), then SIGTERM the Graph process and its live step Processes, each
  only after its recorded identity checks; 20 s, then refuse "did not stop".
  No process-tree walking.
- An unowned Task is claimed, then handed off; one already handed to M
  reuses that entry (idempotent retry).
- `receive` finds its checkout by repository identity, cloning from the
  carried `origin` under `repo_root` (default `~/src`) and binding it when
  M has none.
- Log fallback: `send` carries the rendered log; `receive` starts from it
  (`conversation_log::continue_from` + a new input) when M reads no turns
  from the placed transcript. A provider that refuses at launch is not
  detected (see questions).
- `--machine` stays local to a command that declares it
  (`navigation::machine_invocation`), so `lf task move X --machine M` is not
  SSH transport.

### Decided in review (Jack Heart, 2026-10-26)

1. **History crosses Machines as the native transcript** (Q1 "yes"). One
   carrier for every same-harness move, account or Machine: the provider's own
   session file, same AgentSession id. The rendered log is only for a harness
   change (`-a codex` on a Claude conversation) and the fallback when the
   target's provider refuses the transcript. `Move.history` is
   {harness, agent_session_id, transcript bytes (+ Claude's sibling `<id>/`
   directory)}. Evidence: kickoff's empty-home probes for Claude and Codex,
   and this conversation's probe of a real transcript copied under a
   *different* cwd's project slug ("Not logged in" = found; a bogus id says
   "No conversation found"). Remaining risks: provider version skew on M and
   Codex encrypted reasoning across ChatGPT accounts; the fallback covers both
   and the demo checks the second.
2. **`lf machine use --session` carries the conversation** (Q2 "smoother =
   better"). Moving a Wave or repository primary runs the Session move
   instead of completing here and starting fresh there.
   `move_primary_session`'s complete-and-forget branch is deleted.
3. **A named login that cannot launch fails, naming it** (Q3 "fail").
   When `resume --account` selects a login other than the conversation's
   pinned one, `Prefer` does not fall back to the old login for that launch.

### Concept review against the build (2026-10-27, headless)

Every accepted decision is built. `lf session receive --check` is plumbing
behind the pre-handoff check; no user learns it.

Built 2026-10-27: `lf help resume` lists the account move, the
conversation-only Machine move and `lf task move`; `docs/subscriptions.md`
says `--account` on `lf resume` re-pins the conversation and a named login that
cannot launch fails.

No open product choice; `scratch/questions.md` holds only demo-decided risks.

### Second pass against the build (2026-10-27, headless)

Built 2026-10-27: `lf --machine M resume --task X` naming a Task owned
elsewhere runs `session_move::move_task` (the `lf task move` path), then
forwards `resume --task X` to M. The refusal stays for other forwarded
commands and for `--context`/`--explain` previews, which move nothing;
`lf resume <session>` moves only the conversation. Test: `resume_task_on_another_machine_moves_the_task_there`.

`lf resume --machine M` already parses like `--account` (`reorder_args`
lifts any non-local global flag). Nothing else changes.

## Alternatives considered

- **Rendered log for Machine moves** (the Task text's "copies the Session
  log"). Superseded by Jack's Q1: the native transcript is the log, and the
  rendered log is the fallback.
- **Keep `lf machine use --task` as an ownership-only, offline handoff.**
  Superseded: `lf task move --machine` is the one Task-to-Machine command.
- **Refuse or wait on a live Graph during a Task move.** Rejected by Jack:
  the move stops it; the checkpoint keeps its edits.
- **Hand off the Task whenever its Session moves.** Rejected by Jack: a
  Session move carries the conversation only; the Task move is the API for Tasks.
- **Carry the log in an env var on the interactive ssh call.** Linux caps one
  env string at 128 KiB; logs exceed that. Rejected for stdin on a separate request.
- **Find a moved Session through Task ownership only.** Fails for Sessions
  without a Task and for a Task Session moved alone; the source Session row
  records the target Machine instead.

## Contract

- **Source of truth:** Task ownership log (where a Task runs); the Session
  row on its current Machine (identity, name, inputs), with the source's
  completed row recording where it went; `task_workflows` /
  `task_workflow_moves` on the Task's owner (position, history); provider
  transcript (history); `provider_session_accounts` (account).
- **Surfaces:** global `--account`/`--machine` with `resume`;
  `lf task move --machine`; new `lf session send` / `lf session receive`
  (visible, `after_help` names the two everyday commands);
  `SessionRecord.account`; `lf machine use --task` removed;
  `docs/lf-reference.md` regenerated; `lf resume` and `lf task move` help examples.
- **Absent and error states:** mid-operation worktree → refuse, name state;
  push rejected → refuse, nothing handed off; transcript missing → refuse with
  the Gap message (no empty conversation); M unreachable, an older lf without
  `receive`, or a Task M does not know → before `send`, `relay`/`relay_task`
  run `lf session receive --check [--task X]` on M (skipped when M is this
  Machine) and exit naming M and the cause; nothing is committed, pushed,
  stopped or handed off. `receive` fails after handoff → M owns the Task,
  and the initiator prints the retry to run where the work lived,
  `lf session send <session|--task X> --to M | lf --machine M session receive`.
  `send` is idempotent: already-clean, already-pushed and already-handed-to-M
  steps are skipped and the transcript is re-read on S. Task owned by another person →
  existing handoff refusal. Task Session moved alone → conversation only; Task stays.
- **Operational:** one `ssh::request` per side, bounded by its 20 s timeout plus
  push time; transcripts up to tens of MB over stdin (Claude tool output).
- **Exclusions:** gitignored files; unsent
  input; Desktop "Move to Machine" UI; LOO-618 takeover defaults; LOO-625 Wave
  defaults.

## Delete — do not maintain

All deleted as of 2026-10-27 (`rg` finds none); kept so nothing returns.

- `lf machine use --task` and its hint path: replaced by `lf task move --machine`.
- `move_primary_session`'s "completed here; it continues on M" branch:
  replaced by the Session move.
- `ProviderAccountRoute::resume_requested_session` (field, accessor, `resumed`
  computation in `resolve_selected_provider_account`) and the session-dropping
  branches in `harness/claude.rs` and `harness/codex.rs` `start`.
  `session_resume_is_pinned_by_account_only` / `ProviderAccountSelection.resume_requested_session`
  follow if nothing else reads them.

## Forbidden outcomes

A second continuation renderer; a fresh Session id on the target; a Session
live on two Machines; committing during a merge; a fallback that launches an
empty conversation; a Task move that leaves the Workflow position behind; a
second Task-to-Machine implementation beside `relay_task` (`lf --machine M resume --task X` calls it).

## Slices

1. **Account adoption.** Transcript copy, deletion above,
   `SessionRecord.account`, `-a` narrowing a bare selector. Focused test: an
   isolated Claude and Codex conversation pinned to `first`, resolved with
   `--account …=second`, resumes the same id with the transcript in
   `second`'s home and the pin moved; a named login that cannot launch fails.
   Built 2026-10-26 (c3c298cf4c): `ProviderAccountRoute::adopt` +
   `transcript::copy_between`, `resume_requested_session` and the harness
   session-dropping branches deleted, `SessionRecord.account` with DTO
   fixtures. `-a` narrowing needed no new code: resolution is already
   per-provider, so a Claude login on the same email never competes for a
   Codex launch (asserted in the test). Codex and Claude unit tests both
   exist (the Claude one carries the sibling `<id>/` directory).
2. **Session move.** Built 2026-10-26/27: `ops/human_session/moves.rs`
   (`send`/`receive`/`Move`), `lf/commands/session_move.rs` relay,
   `lf session send|receive`, `--machine … resume <id>`, forwarding of
   `lf resume <id>` and of `lf resume --task X` (a Task Session moved alone)
   on the source, the old client's Machine and account "moved to" lines,
   `lf machine use --session` through it, the log fallback, and cloning when M
   has no checkout.
3. **Task move.** Built 2026-10-27: `lf task move X --machine M`
   (`moves::send_task`, `relay_task`), `Move.task`, `place_moved_workflow`,
   `ingest_ownership_marker`, `check_out_moved_task` (`prepare_task` with an
   explicit source); `lf machine use --task`, bare `machine use` handing off a
   worktree's Task, and their hints deleted; docs and reference regenerated.

## Done when (gate, headless)

- `cargo test -p loopflow provider_account`: isolated move adopts the
  transcript for Claude and Codex (both exist); missing
  transcript errors; a login on the same email under the other provider never
  competes (exists).
- `cargo test -p loopflow --test integration machine_commands` on
  `preview_machines()`:
  - `resume_moves_a_session_to_another_machine`: dirty worktree → commit on
    origin; Session with the same id and name on mini in a worktree of the
    branch, its transcript in mini's provider home under the same
    AgentSession id; `lf resume <id>` on the laptop forwards; Task (if any) still owned
    by the laptop.
  - `task_move_carries_ownership_workflow_and_session`: ownership on mini,
    Workflow at the same node with its history, Session on mini, laptop
    `lf task status` and `lf resume --task` route to mini.
  - `resume_task_on_another_machine_moves_the_task_there`: the
    `lf --machine mini resume --task X` spelling reaches the same state.
  - `task_move_stops_a_live_graph`: a running Graph fixture is stopped, its
    worktree edits land in the checkpoint commit, and the Task arrives on M
    on the same edge.
  - mid-merge fixture: both moves exit non-zero naming "merge", nothing changed.
  - `task_move_hands_nothing_off_when_mini_cannot_receive_it`: a Task mini
    never synced, then an unreachable mini; each exits non-zero, the worktree
    stays uncommitted and the Task owned here.
- DTO fixture tests (Rust + Swift) for `move.json` and `SessionRecord.account`.
- Demo: two real Claude logins and two real Codex logins on one Machine
  (verifies the Codex cross-account risk), then `lf task move` laptop → mini
  with a real Task and keep talking to it there.

Check results: 2026-10-27 (after f7d82445ea) — cargo fmt --check, `cargo clippy --all-targets -- -D warnings`, `--test integration machine_commands` (37 pass, incl. resume_task_on_another_machine_moves_the_task_there): pass. Demo (real logins, laptop → mini) remains with the demo step.
