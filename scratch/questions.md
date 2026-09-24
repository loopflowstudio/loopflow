# Material decisions and unresolved evidence

- This Run is the pinned kickoff contribution. The supplied steer authorizes
  designing LOO-285 here; it does not require starting sibling work, changing
  the accepted serial Reliability allocation, or launching a release now.
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
