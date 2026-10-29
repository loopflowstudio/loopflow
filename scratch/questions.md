# LOO-618 assumptions (headless run, 2026-10-28)

- `--fork` is harness-agnostic: a new Session in the same Work seeded with the source's
  conversation log (`conversation_log::continuation`), not Claude's `--fork-session`.
  Reversible: a native fork per harness could replace the seed later.
- `lf session new` takes an optional exact ID besides `--task/--wave`, so Desktop can
  restart any Session (it used scope selectors and refused non-primaries before).
- Takeover of a client lf did not start (raw `claude`/`codex` still open): lf can't
  identify its process, so `lf resume` continues anyway; docs tell people to close it or
  `--fork`. No signalling of foreign processes.
- `--observe` (read-only attach) is gone with `session attach`; the transport protocol
  still supports passive viewers. Nobody in Desktop used it.
- `.lf/wave/product/MEMORY.md` still names `lf connect`; Wave memory is read-only here.
  Proposed: replace with `lf resume` at the next Wave maintenance pass.
- LOO-424 (resume any conversation from anywhere) is covered by `lf resume <provider-id>`;
  closing it into this Task is left to the Task's operator.

## Accepted (Jack Heart, 2026-10-28)

- Harness-agnostic `--fork` (conversation-log seed) is fine for now.
- No signalling of raw `claude`/`codex` clients lf didn't start; `lf resume` continues and docs point to close-or-`--fork`.
