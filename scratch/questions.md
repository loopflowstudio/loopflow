# LOO-819 implementation assumptions

Explore is open; no vision has been selected. See `explore.html` and the
Explore section of `universal-server.md` for questions 1 and 2. The kickoff's
following choices are proposals:

- Keep the existing `lf server run` installation and LF-owned PTYs. Native Rex
  launch remains an investigation; no program/icon change is promised.
- Start with four projection workers, 256 queued keys and 128 client slots;
  measure them against the stated freshness and whole-process CPU targets.
- Keep thirty-second filesystem reconciliation and three-second degraded
  watching until measurements justify another value.
- Treat the CPU/latency numbers in `universal-server.md` as proposed completion
  targets, not measured improvements or quoted product requirements.

# LOO-823

- `native input` holds 78 of ~100 resident owners for days: any byte the
  provider never acknowledged as one exact prompt (arrow keys, Esc, an edit)
  keeps the owner past `end_detached_after` and rollover. Left as designed
  (`store/sqlite/native_input.rs`); whether terminal-only keys like arrows
  should count as a draft is open.

## LOO-823 open: which input holds a detached owner

Today any byte that isn't plain text or a recognized terminal reply makes the
owner's input `Uncertain`, which holds it past `end_detached_after` until
input is cleared. 78 of ~100 resident owners on 2026-11-10 were held this way.

1a. Keep it: any unknown key holds the owner; the troubleshooting doc tells
    people to `lf resume` and clear it.
1b. Cursor and navigation keys on an empty editor (Left, Right, Down, Home,
    End, PgUp/PgDn) leave input empty; Up, Esc, edits and text still hold
    (Up recalls the last prompt in Claude).
1c. Keep 1a's rule, but an owner held only by `Uncertain` (no recorded text)
    ends at rollover after a fixed age; a typed draft still holds.
1d. Keep 1a's rule and show "held by input" in `lf process list` and Desktop;
    nothing ends on its own.
