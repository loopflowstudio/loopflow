# Material decisions and unresolved evidence

- This Run is the writable human design review for LOO-285. It does not start
  sibling work, change the accepted serial Reliability allocation, or launch
  a release. Human approval/iteration remains on the existing session surface.
- The human accepted catch-up and explicitly allowed “exactly 1 run” that
  automatically collapses misses. Use one release execution per wake for the
  due set frozen at entry. Preserve every due time, link collapsed entries to
  the owning execution, and leave newly due work for the next wake. One shared
  result counts once toward the two-settlement proof. The implementation detail
  is to retain an existing candidate's owner; otherwise the newest outstanding
  due opportunity owns the execution. This needs no per-day replay loop.
- Keep installed daily 09:00 telemetry and 10:00 release schedules. Proposed
  `on_time` display means start during the scheduled calendar minute, with
  exact delay always retained; later automatic starts are caught up.
- Keep the original 35 failed telemetry targets as counterevidence. Fresh
  evidence contains 36. The latest scorecard still queries `agent_turns`, absent
  from the current store. This is a demonstrated verification blocker, not a
  demonstrated present-day artifact-publication failure. LOO-285 owns making
  its release acceptance dependency explicit; an Intelligence repair handoff
  is proposed, not performed or accepted by another owner.
- `release/UI_HOST_GATE.md` declares a required host UI gate. No fresh gate run
  was performed. The design preserves its required status and demands current
  evidence rather than inferring permission or an exemption from old notes.
- Public v0.12.19 existence is reported by `lf release status`; the publisher
  receipt has hashes/stages. No live asset download/smoke was performed, and
  no two-opportunity proof is claimed.
- Historical timezone/trigger provenance is incomplete. Do not backfill it
  from the current timezone or assume all `source=scheduled` receipts were
  autonomous. Preserve unknown coverage before the observation frontier.
- The source's `sync_main` can leave edits in a stash. The design removes that
  call from release selection rather than reopening historical LOO-266 or
  changing the shared helper's unrelated callers.

## Implementation decisions

- The active Run is now the implementation step. Human review's one-execution
  catch-up decision is retained; the opening review-only note describes the
  preceding session.
- Store each obligation and all coalescing links in one atomic JSON replacement.
  This is the same accounting owner with a smaller crash surface than separate
  per-opportunity files. Keep the private Home file store and schema-1 cron
  history; no database migration or scheduler replacement is introduced.
- Current telemetry is checked before selection. The accepted design supersedes
  the earlier no-retry draft: a missing/failed current prerequisite now receives
  at most one automatic retry per release wake through the installed executor.
  The attempt reserves that receipt before launch and retains original failures.
  Running prerequisites defer. The existing executor is synchronous; bounding
  target duration and recovery after its controller dies remain interruption
  obligations, not proof supplied by the retry-count limit.
- Original prerequisite associations use observed release timezone and the
  unchanged installed telemetry schedule. Dates preceding that observation or
  installation stay explicitly unknown. Retaining and reconciling previous
  telemetry schedule/Home segments is still required; current installation
  cannot reconstruct their authority.
- `doctor` still judges scheduled firings, not recovery process success. A missing
  scheduled receipt can therefore remain a continuity failure inside the real
  telemetry retry. The simulated verifier proves retry mechanics, not that the
  actual telemetry flow will pass. No continuity exemption is introduced.
- Disposition writes require an existing local Task Work id plus explicit reason
  and Wave. They record repair ownership without claiming a remote handoff.
- Legacy artifact receipts missing required UI proof invoke the exact-source UI
  gate before read-back can qualify; no historical capability gap is assumed.
- No configured two-opportunity proof or deployment is claimed. Preserve the
  observed scorecard blocker and unresolved Intelligence handoff through review.
- Built-CLI read probes reported stale ambient Wave identity, and one concurrent
  journal initialization reported SQLite locked. Requested file-backed history
  still returned all 70 retained receipts. This is separate runtime evidence;
  no registry/auth repair was attempted and no production-journal health is
  claimed.
