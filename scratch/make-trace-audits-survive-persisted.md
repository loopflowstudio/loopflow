# Trace capture restoration

## Finish line

- Persisted `usage` and `turn_usage` records observed on the long-lived ledger
  read as the current `usage_checkpoint` event at one explicit schema boundary.
- Unsupported, partial, and truncated captures remain visible as distinct
  integrity failures.
- This branch's `lf doctor` audits the real migrated ledger without a capture
  schema-decode failure.

## Observations

- The worktree began clean at `e9eea10ba`, one authored commit ahead of its
  merge base. That commit already contains a candidate normalization in
  `trace.rs` plus fixtures for six historical usage shapes.
- The candidate only runs after current `RecordedConversationEvent`
  deserialization fails, and rewrites recognized schema-v1 records in memory;
  the current emitter remains `usage_checkpoint`.
- The candidate requires exact historical field sets and rejects null usage
  values. The ordinary reader still treats only a truncated final JSON line as
  an incomplete tail.
- `cargo test -p loopflow conversation_reader -- --nocapture` passes all four
  focused reader tests, including normalization, corruption rejection, unknown
  schema rejection, and truncated-tail classification.
- `scripts/dev-lf doctor --json` is not real-ledger proof: development source
  provenance selected an empty isolated worktree store at
  `/Users/jack/.lf-dev/worktrees/loopflow-make-trace-audits-survive-persisted-865933b58483/loopflow.db`.
- The installed `/Users/jack/.local/bin/lf doctor --json` reproduces the real
  failure against `/Users/jack/.lf/loopflow.db`: 1,770 integrity failures, with
  `usage` and `turn_usage` decode errors first, while retaining 71 partial
  captures separately. The count is three above the task report's 1,767, but
  the reported failure class is unchanged.
- Installed binary freshness reports the authored LOO-246 commit `b4c13a75c`
  is already merged on `origin/main`; the installed release itself predates it.

## Hypotheses to test

- Confirmed: the six fixture shapes cover the historical schema failures in the
  long-lived ledger. After migrating a SQLite-consistent backup to the
  candidate's `0.12.14.001_release` frontier, the same audit reports no decode
  error.
- Confirmed: exact-field validation preserves corruption visibility while
  allowing the intended schema history. The focused tests reject unknown field
  sets and null values, and the real audit still reports retained partial
  captures.

## Proof still required

- None. Candidate promotion now owns the long-lived artifact compatibility
  proof that the reader restoration originally lacked.

## Restoration proof

- A release-provenance, validation-only build correctly refused to advance the
  shared `/Users/jack/.lf/loopflow.db`, which remains at the installed release's
  `0.12.13.001_release` frontier.
- `sqlite3 /Users/jack/.lf/loopflow.db ".backup '<isolated>/loopflow.db'"`
  produced a consistent copy. Running the candidate with the real
  `/Users/jack/.lf` artifact root and the isolated database migrated only that
  copy to `0.12.14.001_release`.
- The migrated-copy audit read 274,113 rows. Capture failures fell from 1,770
  to 8, with no serialization or schema-decode failure. The audit continued to
  report 71 retained partial captures separately.
- The audit detail prints four examples, so a direct query checked the hidden
  half: exactly 8 invocations are still `capturing` after a terminal Run event.
  Those 8 account for the entire remaining integrity-failure count.
- Review found one compatibility boundary: recognized schema-v1 variants are
  normalized only after current deserialization fails. The current emitter and
  `--jsonl` artifact output remain unchanged, and unsupported schema history
  still errors.
- Candidate preflight now makes one SQLite-consistent copy, migrates it, checks
  placed lifecycle references, and reads every complete conversation named by
  the copy from the real Home trace root. Its verdict distinguishes missing,
  unsafe, unreadable, truncated, unsupported-schema, and corrupt captures while
  reporting partial rows without reclassifying them.
- The real Home preflight migrated its copy from `0.12.13.001_release` to
  `0.12.14.001_release`, resolved 98 lifecycle references, and read 3,677 of
  3,677 complete captures. It retained 71 partial captures separately. The only
  refusal was the expected validation-only build authority.
- That proof exposed a startup-order counterexample: `install preflight` was
  missing from the global promotion commands that bypass machine selection, so
  an older unreadable `switch.json` blocked the read-only audit. Preflight now
  bypasses that state, with an integration test using an intentionally
  unreadable switch receipt.
- Compression made the migrated snapshot the public compatibility boundary.
  Lifecycle and capture audits are nested under one checked candidate result;
  a snapshot-level failure now yields one unreadable result and one refusal
  reason instead of duplicating the same cause across both audits.

## Review evidence — 2026-08-28

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Historical schema-v1 usage is readable | Normalize the six observed `usage` and `turn_usage` shapes at one reader boundary | Exact-field normalization produces current `usage_checkpoint` events only after current decoding fails | `conversation_reader_normalizes_every_persisted_usage_variant`; six-line historical fixture | pass |
| Current vocabulary and strictness remain | Emit only `usage_checkpoint`; reject unknown fields, nulls, and schema versions without defaults | Current wire fixture is frozen; the normalizer accepts only recorded field sets; no legacy DTO or emitter variant exists | `conversation_usage_checkpoint_wire_shape_is_frozen_for_current_schema`; focused rejection tests; source review | pass |
| Broken and partial captures stay distinct | Unsupported, truncated, corrupt, and partial captures must not become readable history | Complete-capture failures carry distinct kinds; partial rows are counted and never opened or reclassified | `candidate_audits_persisted_capture_schemas_on_the_migrated_home_copy`: 4 complete, 3 typed failures, 1 separate partial | pass |
| The long-lived Home survives candidate migration | Audit the real artifact corpus through a migrated SQLite-consistent copy | Candidate preflight migrated the copy, resolved lifecycle references, then read every complete artifact from the real trace root | `scripts/dev-lf install preflight --json`: 3,680/3,680 complete readable, 71 partial, 98 lifecycle references; validation-only authority was the sole refusal | pass |
| `lf doctor` sees no historical decode failures | The shared reader used by doctor must audit the migrated real ledger | Doctor and preflight share `read_conversation_status`; classified errors only preserve failure kind for promotion | Prior migrated-copy `lf doctor`: 274,113 rows, zero serialization/schema failures, 8 lifecycle failures | pass |

Review found and fixed one missing proof: promotion classified corrupt captures
but its integration fixture exercised only unsupported-schema and truncated
failures. The fixture now includes a structurally corrupt persisted event and
asserts that the partial invocation is absent from the complete-failure set.

# 5 Whys: usage unification made persisted traces unreadable

## The Problem

`lf doctor` could not audit the long-lived Home because complete capture
artifacts written under two earlier usage vocabularies no longer deserialized,
even though every SQLite migration had applied successfully.

The real artifact corpus contains 215 conversation files with outer `usage`,
1,566 with inner `turn_usage`, and 1,108 with the current
`usage_checkpoint`. The task report counted 1,767 integrity failures; the later
reproduction counted 1,770, while the candidate separately exposed 8 lifecycle
failures. The aggregate count is therefore not a stable schema-failure count.

## Chain

Current-only Serde reader → two persisted discriminators changed under schema
version 1 → the version was metadata rather than reader dispatch → verification
moved from the long-lived ledger to a fresh Home → file artifacts had no schema
lifecycle owner

**Problem**: `lf doctor` rejected historical `usage` and `turn_usage` records
instead of auditing the captures that contained them.

**Why 1**: `read_conversation_status` directly deserialized every JSONL line
into the current `RecordedConversationPayload` and `ConversationEvent` enums.
Once those enums stopped naming the historical variants, Serde failed before
the audit could distinguish a valid old record from corruption.

↳ *Could we have caught this earlier?* Yes. One old artifact or the six exact
fixture shapes fails immediately under the pre-fix reader.

**Why 2**: commit `3c59a7f68` removed the outer
`RecordedConversationPayload::Usage` variant and renamed inner
`ConversationEvent::TurnUsage` to `UsageCheckpoint`, while
`TRACE_SCHEMA_VERSION` remained `1`. The same change carefully migrated usage
inside SQLite, but supplied no migration or reader normalization for the
immutable JSONL written beside it.

↳ *What process allowed this?* The change's tests proved the new emitter,
checkpoint table, and current reader together. None supplied a record emitted
by the previous code.

**Why 3**: schema version 1 was not executable. The writer stamped the number,
the reader accepted it as an ordinary `u32`, and no version dispatcher or wire
fixture constrained enum changes. Current Rust domain enums therefore doubled
as a lifelong persisted wire contract without carrying that responsibility.

↳ *What assumption was wrong?* The usage design explicitly required stopping
old writers and said not to add old/new compatibility. That is correct for
concurrent writers crossing a SQLite migration, but it was also applied to
historical readers. Stopping an old writer does not rewrite an immutable trace.

**Why 4**: the feedback loop removed the only data capable of disproving that
assumption. The initial trace guidance required `lf trace` and `lf doctor`
against the long-lived ledger; `3c59a7f68` changed it to a fresh local ledger.
A fresh ledger emits only `usage_checkpoint`, so this exact incompatibility
cannot appear. Candidate promotion likewise copies and migrates SQLite and
validates executable Work references, but never opens the capture artifacts
named by that copy.

↳ *Why was that assumption encoded?* Store compatibility was defined by the
SQL migration frontier. The external artifact tree was treated as output of
the current model rather than another persisted schema with its own reader
lifetime.

**Why 5 (Root)**: Loopflow had no ownership rule coupling a capture artifact's
emitted discriminator, schema version, historical normalization, and
long-lived proof. SQL schema changes had migrations and promotion gates; JSONL
schema changes were ordinary Rust refactors. The system promised lifelong
traces without making artifact compatibility part of the release boundary.

## Unanswered Whys

| Branch Point | Unexplored Question | Priority |
|--------------|---------------------|----------|
| Why 3 | Why was `ConversationEvent` reused directly as the persisted DTO instead of separating the current domain event from the versioned wire shape? | Medium |
| Why 4 | Why did the usage cutover's Done When retain a long-lived installed demo while `TESTING.md` changed the schema proof to a fresh ledger? | High |
| Why 4 | When did `telemetry-daily` first report the permanently red capture check, and why did that signal not route a repair earlier? | Medium |
| Why 5 | Should promotion audit every complete capture or a bounded compatibility corpus once the Home grows beyond a cheap full scan? | Low now; measured full audit is still practical |

## Fixes

| Level | Fix | Prevents |
|-------|-----|----------|
| Immediate | Normalize the six observed schema-v1 usage shapes at the shared reader boundary, with exact fields and no defaults. Shipped in `b4c13a75c`. | The current 1,700+ decode failures without hiding corrupt records |
| Structural | Make trace schema versions executable, freeze the current usage wire shape, reject unknown versions/fields/nulls, and route audit plus human trace rendering through the versioned reader. Shipped in `b4c13a75c`. | Another silent discriminator rename within the covered usage contract |
| Systemic | Make candidate promotion audit the migrated Home copy's complete captures against the real artifact root, and replace fresh-Home schema guidance with that proof. | Any future artifact schema change that unit fixtures or SQL validation miss |

## Changes to Implement

- [x] Restore historical schema-v1 usage variants through one exact reader
  normalization boundary.
- [x] Extend candidate compatibility validation to read every complete capture
  named by its SQLite-consistent migrated Home copy from the real trace root.
  Reject unsupported schema separately from lifecycle loss or partial capture.
- [x] Replace the `TESTING.md` fresh-Home instruction with the candidate-copy
  proof, and add an integration fixture showing that historical variants pass
  while unknown and partial artifacts remain visible.
