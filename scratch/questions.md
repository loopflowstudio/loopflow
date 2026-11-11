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
