# e46a80 open questions and assumptions

- Stop records that fail are marked (`<delivery>.store-gap`) and, per Jack's
  steer, recorded as unknown at the next successful boundary: the hook writes one
  `store-gap` observation (first and last miss) into the Session's history before
  clearing the marker. The observation is stored, not yet rendered by `lf session`
  or Desktop; readers dispatch on payload `source`, so it is inert to them until
  one chooses to show it.
- `lf resume` on a superseded idle owner still ends that owner and reopens the
  Session on the installed lf (`ops/human_session.rs attaches`). Read as "lf
  resume taking it over", one of the endings Jack kept; the integration test
  pins it. If resume should instead attach to the old build, that is a change
  to `attaches`, not to the lifetime policy.
- `docs/sessions.md` never described the executable-replacement ending, so it
  needed no edit.
- The realign step of the Task's `code` graph curated
  `.lf/wave/infrastructure/MEMORY.md` (the earlier pass read it as read-only).
  "Idle owners end" no longer names executable rollover as an ending; `lf monitor
  rollover` under "Moving onto the installed lf" remains the one operator move.
- Why quitting on 11-04 left the terminal ignoring Ctrl-C/Ctrl-D is untraced;
  the candidate is a launcher blocked on the Session fence and SQLite busy
  timeout after the client exited. Settlement now returns within 800 ms either way.
