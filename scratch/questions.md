# e46a80 open questions and assumptions

- Stop records that fail are logged and marked (`<delivery>.store-gap`); Jack's
  steer asked for them to be "recorded as unknown at the next successful
  boundary". The marker could drive that write later; this change clears it
  and logs recovery instead. Assumption: a logged gap is enough for now.
- `lf resume` on a superseded idle owner still ends that owner and reopens the
  Session on the installed lf (`ops/human_session.rs attaches`). Read as "lf
  resume taking it over", one of the endings Jack kept; the integration test
  pins it. If resume should instead attach to the old build, that is a change
  to `attaches`, not to the lifetime policy.
- `docs/sessions.md` never described the executable-replacement ending, so it
  needed no edit.
- `.lf/wave/infrastructure/MEMORY.md` is 89 tokens over its 16000 budget. It is
  read-only in this Graph; the Session continuity section could absorb the
  11-04 incident paragraph above it when a Wave step curates it.
- Why quitting on 11-04 left the terminal ignoring Ctrl-C/Ctrl-D is untraced;
  the candidate is a launcher blocked on the Session fence and SQLite busy
  timeout after the client exited. Settlement now returns within 800 ms either way.
