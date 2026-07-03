# Collapse lfd/lfq into `lf`; shrink lfd to a guarded subscription server

## The shape

Three binaries today — `lf`, `lfd`, `lfq` — collapse toward **one binary + one
thin server**:

- **`lf`** is the workhorse. It already does the real behavior (run steps/flows,
  `lf goal` runs a wave loop). It requires shell/binary/ssh access, and that is
  the *only* access model — there is no remote-behavior-without-a-shell path.
  - `lf d …` absorbs **lfd's** store reads/writes and exec-behavior (waves, runs,
    terminal-sessions: list/get/create/launch). These are local/ssh CLI calls.
  - `lf q …` absorbs **lfq's** queue/worker APIs (dispatch, `worker run`).
- **`lfd serve`** shrinks to a *guarded server* whose only justification is **push
  subscriptions** (the event stream Concerto needs for live status/terminal
  output). It exposes a defined, guarded interface to the outside world and
  internally **execs `lf`** — it does not reimplement behavior.

Result: `lfq` disappears entirely (→ `lf q`). `lfd` the fat daemon disappears;
only the subscription server remains.

### Why keep any server at all

Subscriptions. CLI-over-ssh is request/response; Concerto's live wave-status
badges + terminal-output stream are *push*. That is the one thing a transient
`lf` invocation cannot be. Everything else lfd does today (query API, executor
tmux launch, triggers, queue) is exec-behavior that moves into `lf`.

## Near-term (this branch): Concerto basic UX off `lf` alone

The basic UX we built = repo rail → wave list → click → embedded tmux `/goal`
session. It needs exactly two things, no daemon:

1. **Wave list from disk** — `<repo>/wave/<name>/GOAL.md`. Concerto already has
   this path (the authored-placeholder merge); drop the lfd live-wave overlay and
   the list is pure disk.
2. **`lf goal <wave> --tmux`** — spawn a detached tmux session running
   `lf goal <wave>`, print the handle (`tmux_session_name(branch)` =
   `lf-<branch-with-dots-as-dashes>`), exit. Concerto's click becomes
   `lf goal <wave> --tmux` → read handle → `tmux attach` (its existing
   `GhosttyTerminalView` path). No HTTP, no token, no 400.

**Stays lfd-gated (deferred, subscription-backed):** live status badges
(running/failed/waiting), attention queue, cross-repo rollups. The lfd-free basic
UX shows waves-from-disk + attach-to-launched-session; the live-status layer
lights up *when* lfd is connected and degrades gracefully when it isn't.

The `lf goal --tmux` primitive is the foundation — needed in every version — and
mirrors lfd's proven `launch_tmux_session`
(`rust/loopflow/src/lfd/executor/wave/mod.rs:981`): `tmux new-session -d -s
<name> -c <cwd> /bin/zsh -lc "<argv>; record exit"`. v1 can skip the exit-file
bookkeeping (that is lfd's lifecycle tracking); add it back when lfd's
subscription server needs completion signals.

## `lfdb` — the shared backend under `lf d` and `lf q`

`lf d` and `lf q` read/write the same tables (waves, runs, sessions,
terminal_sessions, credentials, queue state). They need one backend, not two
reach-ins. Extract `crate::lfd::store` (backends + migrations + persisted domain
types + the registry API) into **`lfdb`** — persistence as *shared infra*, no
longer lfd-owned. lfd becomes one more `lfdb` client: the one that watches it and
pushes subscriptions.

The session-registry model lands here: `lfdb` exposes `register_session` /
`active_sessions_by_worktree`, every `lf` session self-registers on start, and
the db is the run registry. (The migration-idempotency fix in
`store/migrations.rs` is already `lfdb` code in waiting.)

Boundary calls:
- **Persisted types vs wire DTOs.** `lfdb` owns `Wave`/`Run`/`Session`/`RepoWork`
  (the storage shape). The HTTP DTOs stay with the shrunk lfd server (the
  *subscription* shape), keeping the DTO-drift rules scoped to the one surface
  that still crosses a network.
- **Crate vs module.** Start as a bounded module (`crate::lfdb`, i.e. lift
  `lfd::store` out of `lfd`); graduate to a workspace crate once the seams are
  proven. Rename first, extract when earned.

### Session identity (registry)

- Group key = **worktree name** (dir basename, e.g. `loopflow.goals`).
- Multiple agents under one worktree → differentiate within by session-id/step
  suffix; group by the shared worktree prefix. "Show active agents" enumerates
  live sessions grouped by worktree.
- RESOLVED: `lf goal <wave>` runs **inside the wave's worktree** (`loopflow.<wave>`),
  not `main_repo`. It creates/enters the worktree, runs there, and registers under
  that worktree name. Concerto **pre-allocates worktrees** in some cases (so a
  wave has its worktree ready before its first goal launch).

## Migration posture: hard cut, no compat

This is an internal system — no external DB/API consumers. Per CLAUDE.md, do a
**hard, irrecoverable cut** to the new shape: drop the old lfd-owned store/executor
paths outright rather than carry compat shims or a migration bridge. Existing local
dbs/sessions are disposable. Do not preserve the old HTTP-executor launch path once
`lf` owns tmux launch + registration.

## Ship-vs-defer plan (this branch: Concerto lfd-free basic UX)

**Crux resolved.** Concerto's attach is already `tmux attach-session -t <name>`
(`WavesView.swift:890`, `TerminalWorkspaceView.swift:186`). lfd only *resolves the
name* (`attachSession(id)` → `connection.sessionName`); the attach itself is plain
tmux. Since name = worktree name (deterministic), Concerto can get the name with
zero lfd. The ship slice is small.

### Ship now — "kill lfd, click a wave, watch its goal loop launch + attach"

1. `lf goal --tmux` runs in the wave's **worktree** (create-if-missing), session
   named after the worktree. (Rust — extends the built primitive; today it runs in
   `main_repo`.)
2. Concerto wave-list renders **from disk alone** — drop the lfd live-wave overlay.
   (Swift — authored-placeholder path already exists.)
3. Concerto click → exec bundled `lf goal <wave> --tmux` → read handle → feed the
   existing `tmux attach-session -t <name>`, bypassing `attachSession(id)`. (Swift —
   small; reuses the attach view.)

Proof-of-working: kill lfd, click a wave, the loop still launches + attaches.

### Research to close first

- R1 — worktree create mechanics: reuse `engine::worktrees`; decide branch naming
  (stable `jack-heart.<wave>` vs timestamped).
- R2 — does `lf goal` run to a watchable state in a fresh worktree with lfd/lfq
  absent? (renders prompt + launches agent; verify the loop visibly starts).
- R3 — attach-by-name: DONE (resolved above).

### Design decisions

- D1 — worktree lifecycle: `lf goal` creates-if-missing; Concerto proactive
  pre-allocation deferred.
- D2 — naming rule shared lf↔Concerto: `lf-<worktree-basename>`; Concerto derives
  it to show "running?" via `tmux has-session`. Multiple-per-worktree grouping
  deferred.
- D3 — status without lfd: derive running state from `tmux has-session`.

### Deferred to reduce (not needed for the demo)

`lfdb` extraction · self-registration/registry · `lf d`/`lf q` · the hard cut
(delete lfd's HTTP executor) · subscription live-status · proactive worktree
pre-alloc. The demo runs as **lfd-present-but-unused** (provable by killing it),
so no irreversible cut is required to see it work.

## reduce roadmap seed

The full fold-in is a reduce-wave job (collapse a concept, net-negative code):

- Move lfd query routes → `lf d <verb>` reading the store directly.
- Move lfd executor (tmux/docker launch, triggers, janitor) → `lf` /
  `lf d` exec paths; `lfd serve` execs `lf` for these.
- Move lfq → `lf q`; delete the `lfq` binary.
- Shrink `lfd serve` to the subscription hub + guarded external interface.
- Concerto: `lf d`/`lf q` for reads/actions; keep an lfd subscription connection
  only for push.
