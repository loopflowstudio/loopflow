# W2-202 open questions / blockers

## Blocker: `lf task acknowledge` blocked by forked local store
The local registry `~/.lf/loopflow.db` carries migration `0.11.009_profiles`,
which is unknown to every `lf` binary on this machine (PATH `lf` 0.11.1 knows to
`0.11.012`; fresh `target/release/lf` knows to `0.11.013_task_review_state`).
This is a migration-history divergence, not a stale-but-forward store. So
`lf task acknowledge W2-202 --directive 1 ...` fails with "no Loopflow registry
on this machine". The supervisor already resumed this session with directive v1
incorporated (per wave chat), so the directive is understood; the acknowledge
receipt is bookkeeping I cannot write from here.

Decision: do not repair store/wave identity inside this Task (out of scope, and
the supervisor owns it). Proceed with the clarify+design work, which does not
depend on the store. Ship the code and PR through git/gh directly if `lf pr`
plumbing is likewise store-gated.

## Assumption: title/body generation policy unchanged
Task excludes changing title/body generation. `lf pr publish` reuses the exact
generation path `lf pr open` uses today; it only omits the presentation call.
