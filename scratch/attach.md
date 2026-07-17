# Pauses, Blocks, and the Human Door

## Problem

`lf task attach` drops a human into the steering control terminal — a protocol
surface built for agents. The tmux fallback (preserved sessions per task) has
broken colors, session names to remember, and server state to garbage-collect.
tmux is a debugging tool that leaked into the product API.

Meanwhile the runtime has no noun for "this flow needs a human now." INT-10 is
the live specimen: its kickoff finished and paused for review, but the pause
exists only as prose ("once the human accepts the boundaries, move the map to
docs/") plus a `scratch/questions.md` sidecar where the worker self-resolved a
directive ("directive v2 was the greeting 'hello'; treated as non-substantive")
with no ledger entry anyone would see. The task was framed as an interactive
review and ran headless because interactivity is a compile-time flag, not a
runtime binding.

## The demo

Blue light on a paused task in Concerto → tap → embedded Ghostty terminal opens
the pause's default skill as a real conversation → conversation concludes →
worker incorporates the outcome as a directive and resumes. Colors work; there
is nothing to attach to, name, or clean up.

CLI form (buildable first): `lf queue` lists blocks; launching the block's
default skill in your own terminal is the same action without the window.

## Nouns

- **Pause point** — authoring-time, declared in a flow. Owns a default skill.
- **Block** — runtime instance of hitting a pause point. An addressable context
  handle: worktree, provider session, reason stopped, directive state.
- **Queue** — the set of unresolved blocks. The blue light is its presence
  indicator in Concerto.

Existing scattered material this unifies: task `waiting` + `status_reason`,
`lf handoff list`, gate proposals, `interaction_policy` phases.

## Key decisions

### 1. tmux leaves the product API

Persistence lives in the session record, not the process. A provider session
resumes from its rollout in the account home
(`CODEX_HOME=~/.lf/accounts/codex/<id> codex resume <provider_session_id>` —
verified live against INT-10). The process may die with the terminal; re-launch
is another resume. tmux remains an internal/debugging door into worker
processes, absent from the API.

### 2. Steering protocol stays — agent-only ingress

The directive protocol is the *only* way anything reaches a worker, and only
agents speak it. A human resolving a pause converses with a skill in a real
terminal; the skill translates the conclusion into a directive. From the
worker's perspective a human review session is indistinguishable from a parent
agent: same protocol, same incorporation semantics, same versioned record.

### 3. Kill the `interactive` flag on skills

Interactivity is a property of who holds the other end, not of the skill. Every
pause launches its default skill interactively with *someone*:

- **pause mode**: a human, via the queue / blue light;
- **no-pause mode**: the parent agent stands in as interlocutor.

There is no naked mode — conversation-shaped skills always get a conversation.
Auto-run doctrine ("decide and note in scratch/questions.md") grows up into
blocks with recorded self-resolutions. `interaction_policy` maps to routing:
`require` → always goes blue; `defer` → parent answers, blue only on
escalation (which arrives with the parent/skill transcript attached).

### 4. Any skill can proceed a block

A block is a context handle, so any skill can be launched over it; the
"recommended next step" is just the default skill, not a separate mechanism.
Session identity forks two ways:

1. **Default skill** resumes the worker's own provider session — full
   continuity, bound to its provider/account.
2. **Any other skill** gets block-assembled context (transcript, reason,
   worktree) in a fresh session on any provider.

Resolution is recorded for free: a skill run over a block is a run.
"Resolved" means the worker *incorporated* the outcome (the directive-version
pair already distinguishes this), not merely that someone responded.

### 5. Resuming lease, not exit traps

Concerto launches the default skill with a flag (spelling ~
`lf skill run <skill> --block <id> --lease-resume`). The flag means the run
holds a *resuming lease* on the block, watched by lfd (pid + heartbeat). Any
end of the session — clean exit, Ctrl-C, window close, SIGKILL, app crash —
is one case: lease lapsed → branch unpauses. No cleanup handler has to survive
a crash. Second consumer of the existing lease-broker pattern.

Exit unpauses; exit never concludes:

- **Skill concludes** → outcome written as the directive before exit.
- **Session just ends** → directive is "human engaged, no conclusion —
  transcript attached"; the worker proceeds on what was actually said, or on
  its own recommended default if nothing was.

Bare CLI runs may omit the flag to leave the block parked. The launcher cannot
know *why* a session ended, so the design never asks it to.

### 6. Concerto: blue paused state + embedded Ghostty

Blue joins the light vocabulary as "your turn" — invited, not alarmed;
distinct from working, broken, and waiting-on-external. Tap launches the
default skill in the embedded terminal. `LoopflowMac/Services/Ghostty/`
(libghostty wrapper, surface management, slate theme) already exists; this is
wiring, not new infrastructure. Concerto owns the pty, so colors work.

## What's left to build

1. **Block + queue in lfd** — one noun unifying task-waiting, handoffs, and
   gate proposals; `lf queue` listing with reason + default skill.
2. **Proceed path** — launch a skill over a block; default-skill path resumes
   the worker's provider session from its account home.
3. **Lease** — `--lease-resume`, lfd lease watch, lapse → unpause; the
   two-directive exit semantics.
4. **Pause points in flows** — declared with default skills; delete the
   `interactive` flag; no-pause mode binds the parent as interlocutor.
5. **Concerto** — blue state on the indicator; tap → Ghostty launch with the
   lease flag.
6. **Retire `lf task attach`** from the product surface (debugging door only).

## Open questions

- Does an outstanding block halt the whole flow or only its branch? Same
  question as "what may proceed under an unincorporated directive" — answer
  once, in one place, for both.
- Queue ordering when several blocks are live: recency is easy, wave priority
  is what you'd want. Who ranks?
- Mid-turn spectating (watching a live turn stream) is not covered by
  resume-at-rest; if it matters it's a separate read-only streaming feature,
  not a reason to keep tmux.
