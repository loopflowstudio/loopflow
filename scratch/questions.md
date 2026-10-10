# Implementation choices — LOO-412, 2026-10-09

Jack Heart selected repository-wide configuration, personal Git by default, shared
Git or Linear, and required one typed planning writer. Linear outages buffer locally
without transport fallback. These decisions supersede manual per-Wave selection,
callbacks, a designated publisher and mixed Git/Linear receipt replication.
The [design](work-on-another-machine-name.md) owns current remaining work.

- Personal keys are generated once locally and recovered on another fresh Machine.
  Automatic account-based key discovery/pairing is not implemented or claimed.
- Configuration selects the repository's entire planning history. Shared-ref
  permission comes from that setting, not membership inference or a Machine label.
- Switching an existing Git plan to Linear keeps records and old receipts, but
  automatic export of peer-imported unmapped records needs an explicit design.
  Preserve uncertain effects rather than inventing cross-provider settlement.
- Git causality precedes logical clocks and stable mutation-ID ties. All losing
  values remain. No initiating host has blanket preference over another writer.
- Import preserves local execution. Missing placement proves nothing about work
  on another Machine; this Task builds no distributed first-start authority.
- One final `planning_peers.sql` draft depends on LOO-406/optional-PR schema.
  Experiments use fresh disposable stores; no branch CLI writes the installed Home.

Earlier choices/evidence remain at
`ae96102580e442fe582dd17ce5e6fef9b8cee69d:scratch/questions.md` and the design's
historical references. They do not reinstate superseded mechanisms.
