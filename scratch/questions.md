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
- Only `planning_peers.sql` remains a draft after `db36f20ba8`; LOO-406/optional-PR
  schema is released in `0.13.11.001_release.sql`. Experiments use fresh disposable
  stores; no branch CLI writes the installed Home.

Earlier choices/evidence remain at
`ae96102580e442fe582dd17ce5e6fef9b8cee69d:scratch/questions.md` and the design's
historical references. They do not reinstate superseded mechanisms.

Sync resolution (2026-10-10): retain main’s checkout-owned Wave Markdown and
new AgentProcess/module paths, while preserving the typed planning writer and
optional placement. Wave registration now captures through that writer; Workflow
definitions remain stored separately. No transport or delivery authority changes.

Prose boundary (2026-10-10): an outbound prose replacement refuses live managed
Markdown declarations and unclosed fences rather than changing saved workflow,
KRs or targets. Closed fenced examples remain supported. This is a reversible
implementation choice, not Jack Heart's UX approval. Ordinary saves/readback trim boundary whitespace and join prose segments across
managed spans with a blank line. Migration preserves original bytes. Partial
observations retain prose value/revision/age separately from newer entity facts;
this field evidence is not a second Work writer. These are reversible implementation
choices, not additional UX approval.
