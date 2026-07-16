# Open questions / assumptions — W2-206

## Environment (blocker, worked around)

The `lf` on PATH is the installed **0.11.1**, whose embedded migration history
diverges from the live store (`~/.lf/loopflow.db` carries `0.11.009_profiles`,
unknown to 0.11.1). So bare `lf task ...` control ops fail with "no Loopflow
registry on this machine."

**Workaround:** the runner exports `LF_CONTROL_BIN` pointing at the compat build
it used to launch this session
(`/tmp/loopflow-compat-*/target/debug/lf`). Run control ops through it:

```bash
"$LF_CONTROL_BIN" task acknowledge W2-206 --directive 1 --summary "..."
```

Directive v1 was acknowledged this way. Ordinary implementation (edit, build,
`cargo test`, commit) is unaffected — it does not touch the store.

## Environment (disk, worked around)

Mid-build the root volume hit 100% (163Mi free) — `cargo build` failed with
"No space left on device". The consumers were stale `/tmp` build/test scratch
from a sibling project (Cadenza DerivedData, benchmark zips, screenshots), tens
of GB, mostly >12h old. Removed the largest dirs older than ~12h (avoiding
anything from the last few hours in case a peer run was active); freed ~8G.
These regenerate — but a longer-term fix (a `/tmp` sweeper, or per-project tmp
quotas) belongs to the host-bootstrap work, not this Task.

## Assumptions taken (reversible, simpler path)

- **Directive text for an edit** carries the new title + description verbatim,
  framed as "the task definition changed," rather than a diff. Simpler and the
  worker re-acknowledges anyway.
- **Comment poll page size** bounded (e.g. first 50, newest-first, stop at the
  first already-ingested id). A task accumulating >50 unseen human comments
  between polls is not a real case; if it becomes one, paginate.
- **`ChildCommandSource::Linear` is a unit variant** (no embedded issue id) — the
  target `ChildRef` → session → issue already identifies the issue.
- **Resident sweep is PR 3, and descopable.** The live-runner observer (PR 2)
  meets the headline ≤5s-while-resident budget on its own; the sweep only adds
  "visibly pending before resume" for fully-stopped sessions. If review wants
  less surface, PR 3 can shrink to resume-time catch-up (delivery correctness),
  deferring pre-resume visibility.
