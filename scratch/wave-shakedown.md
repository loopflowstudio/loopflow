---
requires: wave-agent-design.md
produces: the live-vendor shakedown runbook — Claude-drivable, no daemon
---
# Wave agent shakedown

## Results (2026-07-04, first walk — gates 1–4, Claude-driven)

**PASSED live:** worktree self-bootstrap; endpoint discovery; full codex
handshake + real turns streaming item-by-item through the open-turn wire;
message-while-idle answered immediately; **steer landed mid-turn**
(`turn_steered` journaled, answers named); **interrupt finalized the partial
as `interrupted`** with the state machine walking
turning→interrupting→idle in <3s, vendor session surviving; **restart
replayed all five turns intact** with statuses preserved; teardown left zero
orphan processes. The mind adapted when dispatch failed (probed, then did
trivial work inline) and committed real work (HAIKU.md).

**FOUND (fix batch dispatched):** (1) mind's PATH resolves the system `lf`,
not the running build → `lf q` unknown; (2) unregistered waves (no store
row) skip one-brain entirely — two live brains observed; (3) second server
clobbers then deletes `.wave-endpoint`, orphaning the first from discovery;
(4) empty `thought` items on the wire; (5) operating prompt needs the
triviality escape hatch (five probe commands before inlining a one-liner).
Earlier same day: codex-cli 0.142.5 protocol drift (5 shapes) + two shutdown
bugs (nvm-shim grandchild orphan; reader/writer deadlock) — all fixed,
proven by a 3s live smoke turn.

**Gate 5 (second walk, same day): PASSED — and richer than scripted.** The
mind dispatched via `lf q worker run` (one clean turn, run id reported
back); worker landed in `<repo>.demo.<runid>` + tmux; `worker_dispatched`
journaled by the store observer; the worker sent FOUR attributed `lf chat`
reports mid-run; each entered the mind's queue and was answered; the mind
ran `lf memory add` unprompted (memory_updated journaled); `worker_finished`
closed the loop; the thread shows `from: worker` bylines throughout. Found
along the way: drift #2 (`wave_runs`/`wave_run_id` — the mind diagnosed the
missing `runs` table itself by reading our source and diffing schemas;
healed by migration 049 after a full fresh-vs-live schema diff), and the
SIGHUP teardown gap (tmux kill-session bypassed SIGINT-only cleanup,
orphaning the mind's codex pair — fix in the wrap pass).

**Gates 6–7 (first walk, same evening — Jack driving the REAL goals wave):
chat through Concerto WORKS** ("that worked. at least the chat worked
fine"). The walk found five more live bugs, all fixed same-session: the SSE
blackout (URLSession's AsyncBytes.lines silently drops the empty line that
terminates an SSE frame — no streamed frame ever dispatched; replaced with a
real incremental parser, pinned by a verbatim captured frame); Concerto
launched at a worktree reading wave state there instead of the origin
(WaveOrigin.resolve mirrors Rust wave_origin); the Start button resolving a
stale system lf (capability probe `lf help wave` — `lf wave --help` exits 0
on stale builds); server-created wave rows born loopable under legacy
tickers (born paused now); and the near-silent console (narration pass).
Meanwhile the wave itself worked the whole time: read the live roadmap,
autonomously dispatched a kickoff-design worker then a well-briefed build
worker for "Prove the language" (falsification framing, write-gaps-back-to-
Asana, owned worktree), answered chat with live status. Bonus find at
teardown: YESTERDAY'S old-style goals agent was still alive in the shared
worktree — two brain generations on one tree, the class one-brain now
prevents. Remaining judgment gates: steer + interrupt from the composer
mid-turn, and the leave-it-running-overnight call.

Code-complete ≠ done. Everything past the conformance traces is untested
against live vendors. No `lfd` anywhere in this runbook — the wave server is
sovereign; the store is the registry. Gates 1–5 are Claude-drivable from the
CLI (background tmux + curl + journal tail); gates 6–7 need Jack.

## Rules of engagement
- First live-mind runs happen in a **throwaway repo**, never this tree — an
  AutoApprove codex grinding in the worktree we're editing is self-inflicted
  split-brain. Setup: init a tiny git repo in the scratchpad with
  `wave/demo/GOAL.md` ("maintain TODO.md; one small improvement per pass")
  and a MEMORY.md stub.
- Watch spend: every mind turn burns a subscription turn. Interrupt early,
  keep passes short, tear down when idle.
- Journal is the oracle: `.lf/journal/waves/<name>/journal.jsonl` in the
  wave's worktree. `/health`, `GET /conversation`, and the SSE stream are the
  wire views. Kill switch: Ctrl-C on the server; verify no codex survivors
  with `pgrep -fl codex`.

## 1. First contact — the mind alone (throwaway repo)
- [ ] `lf wave demo` → bootstraps/enters `<repo>.demo` worktree, writes
      `.wave-endpoint`, registers a WaveAgent session row in the store.
- [ ] Journal order: ThreadStarted first, then MindState idle→turning,
      TurnStarted…; `/health` walks idle→turning.
- [ ] First real codex app-server turn streams items and finalizes.
- Likely bugs: handshake details the traces missed; thread-id capture window;
  first-turn seed size; codex flags drift.

## 2. Chat (curl-drivable)
- [ ] `{op:"message"}` while idle → immediate turn, answers names the message.
- [ ] Message mid-turn → queued, boundary turn drains it (answers = all ids).
- [ ] Server restart mid-conversation → thread intact, ids continue, vendor
      thread cold-starts (documented).

## 3. Steer + interrupt (the new muscles)
- [ ] Mid-turn `{op:"steer"}` → lands in the live turn; TurnSteered journaled.
- [ ] Mid-turn `{op:"interrupt", text:""}` → turn finalizes `interrupted`,
      state walks Turning→Interrupting→Idle, `pgrep codex` clean.
- [ ] Interrupt & send → next turn answers the text.
- [ ] Interrupt during a codex tool call → does the 10s deadline force-path
      fire cleanly?

## 4. One-brain + registry (store-direct)
- [ ] Second `lf wave demo` refused, names the live session; `--force` takes
      over; dead-pid row → takeover without --force.
- [ ] Session rows visible in the store (sqlite3 query or `lf d` reader when
      it exists); wave server row marked terminal on Ctrl-C.

## 5. Orchestration — workers, no daemon
- [ ] Ask the mind (chat) to dispatch → it runs `lf q worker run demo …` →
      worktree `<repo>.demo.<id>`, detached tmux session, run+session rows.
- [ ] Worker's own `lf` self-registers (child session row, correct parent;
      no double-registration).
- [ ] WorkerDispatched appears in the journal (store poll); WorkerFinished on
      completion; next heartbeat turn carries <in_flight>.
- [ ] Placement: `--stack` forks from parent branch with lineage columns;
      pooled run shares the wave worktree.

## 6. Concerto (Jack)
- [ ] WaveChat attaches via .wave-endpoint; turns stream live; composer verbs
      follow state (Send / Steer / Interrupt & Send / Interrupt).
- [ ] Kill the server → sane degrade; restart → clean reconnect.
- [ ] Judgment gates: does steering *feel* immediate? Is the transcript
      readable at a glance? Would you leave this running?

## 7. The real thing (Jack + Claude)
- [ ] `lf wave goals` in its own worktree, the actual goals GOAL.md, real
      Asana roadmap via `lf op pm`. One supervised session: watch it read the
      roadmap, dispatch one real worker, land one real PR, update Asana.
- [ ] Soak an hour: heartbeat cadence sane, MEMORY.md gets *curated* edits
      (or the operating prompt needs tuning), journal growth reasonable, no
      fd/process leaks, spend acceptable.

## Known-accepted gaps (don't re-find these)
- Vendor thread resume = cold start (visible thread survives via journal).
- SSE terminal-frame drop on a lagged client shows `running` until reconnect.
- WorkerFinished.summary is thin (exit + PR url) until worker reports exist.
- Steer degrades to queue on claude/opencode minds (codex-only for now).
- No Decisions/gating; ApprovalPolicy is AutoApprove.
- Concerto fleet surfaces (wave list badges, agent tree) still read the old
  daemon — they need `lfd serve` (subscription server) or its successor;
  WaveChat itself is daemon-free.
