# Compress ordinary Flow boundary ownership

The captured Flow definition and ExecutionCursor already select the current
occurrence. Boundary duplicates that occurrence's name, human policy and
decision policy, supplied again by the CLI at launch. Remove those copies and
derive them from the saved occurrence in every decision, recovery and Session
reader. Boundary retains only attempt identity, provider binding, completion
and readiness. Task transactions and ordinary file locks remain separate owners.

Finish line: one source for occurrence policy; callers cannot assign a second
policy when beginning a boundary. Existing saved positions remain readable,
including nested human approval and pending autonomous decisions. Old redundant
JSON fields may be ignored; captured definitions and exact tokens must survive.
Provider exit must still never approve a human gate, and stale decision Runs
must remain rejected.

Proof: focused ordinary Flow persistence, Session and CLI execution tests;
formatting, all-target Clippy and architecture checks. No live provider or human
acceptance is inferred. The Wave interpreter deletion remains separately scoped
because it needs journal and resident/client migration. Persisted legacy Task
decision decoding stays because it preserves recoverable facts.
