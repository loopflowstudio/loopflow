# The lf language

Grammar, dispatch surface, and the channels remainder. Audited 2026-07-08 —
the asymmetry inventory IS the work list:

## KRs

- One way to reach a flow/skill: today there are three (`lf flow X`,
  `lf skill X`, bare `lf X` unchecked); the bare-fallback decision
  (Linear a9dc614d) resolves this.
- `task`/`wave` namespace overlap adjudicated: `lf task` the verb vs `task`
  the flow (`lf flow task`) are different code paths with one name.
- Ledger reads become consistent: `lf usage` goes over HTTP to lfd while
  `lf ls`/`runs` read lfdb directly — one rule for "read the ledger."
- The doubled `-w/--wave` flag (top-level scoping vs WaveTargetArgs
  targeting in chat/memory) collapses to one semantics.
- `lf -l` vs `lf ls`: one listing mechanism.
- The exec-door verb subset (denies wave/task) is documented as the
  intentional grammar of what subagents may drive (Linear 8e01041f).
