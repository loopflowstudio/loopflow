# Compiled checkpoint

`MEMORY.md` is the bounded, curated artifact that survives every boundary —
and post-#845 it is MORE load-bearing: every wave pass seeds from it.
Audited 2026-07-08: the pen is the live server (`runtime.update_memory`,
sole writer; serverless fallback only in export-memory); the compile is a
stateless cron skill (`export-memory`), where the typed-block vocabulary
lives as prompt-only guidance. The wave flowloop reads memory every pass
but never compiles it; `wave_mutate` instructs curation as agent prose.

## KRs

- Answers "what's decided, what's constrained, what's in flight" at a
  glance — typed blocks — and stays prompt-sized as facts accumulate.
- No learning is lost across a land, branch, machine move, or compaction.
- The curation home is decided: scheduled export-memory compile vs
  `wave_mutate` pass vs the steward (goals/flowloop bet) — one owner, not
  three half-owners.
