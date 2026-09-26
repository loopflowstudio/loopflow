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

## Result and review

Removed Boundary.name/human/deciding and the three matching begin_boundary
arguments. CLI launch, autonomous decision/route checks, blocker authority,
recovery, human validation, Session display and saved approval settlement all
read the selected captured occurrence. Removed the Session adapter's redundant
current_body wrapper. No public CLI command or wire DTO changed.

The boundary UUID still distinguishes attempts; Run ID/path preserve execution
evidence; completed distinguishes receipts from pending execution; ready_summary
records human readiness without approval. These cannot be derived from the
definition. Task claim/version transactions, ordinary record/driver locks and
the cursor remain their existing owners. No schema or migration adapter added.

Review found that two fixtures assigned human policy at launch over an autonomous
definition, and an operation fixture used Skill steps. Corrected their captured
definitions so they exercise the real contracts. The historical JSON proof
retains the attempt UUID, Run ID and artifact path and still rejects autonomous
approval of a human gate after provider success. Ordinary serde unknown-field
handling discards the old duplicated fields on the next normal write; no source
reload or proactive saved-state rewrite occurs.

Inspected mirrors: FlowRun/Boundary persistence; CLI begin/decide/route/blocked;
Flow Session validation, listing, display, reopening and settlement; Task
FlowPosition's existing policy derivation; architecture ownership documentation.
Swift consumes SessionRecord, whose shape and behavior are unchanged. The Wave
playhead/type move remains the explicitly deferred resident/journal/client slice.
Run ID and artifact path remain together because recovery needs the original
receipt location, not a guessed current Home path.

Focused verification: nextest 4af8d642-7b62-4ae2-b476-235469585dab,
**12 passed, 0 failed**, using isolated Home/Run authority:

```sh
cargo nextest run -p loopflow --lib -E \
  'test(ops::flow_run::tests::) | test(ops::flow_session::tests::) | test(lf::commands::flow::tests::)'
```

This covers pending decisions, failure/interruption, exact and stale approval,
nested human revision, launch recovery, Session isolation, operation replay
prevention and the real CLI executor's durable human wait. Provider side effects
are simulated. No live acceptance, full branch-suite or hosted UI claim.

All-target Clippy (`cargo clippy --all-targets -j4 -- -D warnings`),
`cargo fmt --all -- --check`, architecture ownership and diff whitespace checks
passed. This compression Run performed no commit, push, installation or production
Session mutation. Final inspection observed concurrent Loopflow checkpoint
86dc0002e (`lf pr open: prepare branch`) had included the Rust reduction and the
initial design note. The implementation remains present; the added documentation
and final evidence are left as working-tree edits.
