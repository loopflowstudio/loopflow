# Open choices (LOO-901)

Assumptions taken in `claude-capacity.md`; each is reversible.

- `--clear-cooldown` releases holds on every window of the login, including
  account-wide ones (`five_hour`, `seven_day`), not only model-scoped
  rejections. The direction says "clears recorded scoped rejection holds
  consistently"; a clear that left some holds would repeat the reported
  confusion. A following refresh re-holds a window that is really spent.
- `lf account next -a claude:<model>` becomes model-specific. LOO-900 kept
  `next` model-unspecified; plain `next` still is.
- Claude's pre-#1828 `weekly`, `weekly:<family>` and `session` rows are treated
  as known historical names alongside the reason-shaped ones. All such live
  rows had passed their reset when read on 2026-11-14.
- No data migration: a still-running pre-#1937 `lf` can write the old names
  again. The classifier's removal, once no such process can write the store,
  is not yet filed as a Task.
- The `launches` field name and its text line are a first shape; the wire
  shape (model, eligible, out_until, blockers) is what the plan depends on.
  `out_until` replaces `held_until` because `AccountRow.held` already means
  pinned by `lf account use`.
- Plain `lf account next` does not read the configured default agent's model,
  so it can report Claude blocked while the next plain launch (say
  `claude:sonnet` by config) would run. Kept from LOO-900.
- The docs show a blocker as "cooling for fable until <time>" and say a paused
  or signed-out login names that on the `launches` line in place of a time.
  Both are a first wording; the code's blocker strings win and the docs follow.

- `launches` checks cached credential identity as routing inspection does; a
  connected credential without a matching identity reports that blocker. The
  server `account.next` request gets required nullable `agent` so model selection
  survives served reads; Swift's request type carries it too.
