# Receipt resolver + doctor sweep — making curated claims drill to evidence

Continuation of W2-124 (PR #919). The receipt contract and memory-fact
binding shipped; this PR adds the drill primitive and the doctor sweep.

## What shipped in PR #919

- `Receipt` / `EvidenceKind` DTO + fixture (Rust + Swift)
- `MemoryAdded.receipts`, `lf memory add --receipt`, `lf memory log --json`
- Receipts journaled alongside facts; read back from the journal fold

## What this PR adds

### `lf receipt show <kind:reference> [--wave NAME] [--json]`

The drill primitive. Resolves one receipt to its canonical local record:

| Kind | Resolution surface | Record returned |
|------|-------------------|-----------------|
| `chat_turn` | wave journal thread fold | turn id, role, text, created_at, from |
| `worker_report` / `run` | run ledger (SQLite) + journal RunCompleted | run_id, outcome, summary, event count |
| `trace` | agent_turns table (SQLite) | turn id, launch id, status, tokens, timestamps |
| `pm` | PM snapshot (SQLite, cache-first) | id, identifier, name, completed |
| `pr` | task_prs table (SQLite) | pr id, branch, slug, phase, github url, merge sha |

All resolutions are local reads. `pm` reads the cached snapshot, never
hitting Linear. Unresolvable references exit non-zero with a reason.

### Doctor receipt sweep

New `receipts` check in `lf doctor`. Sweeps every wave's memory facts for:

- **missing** — fact with zero receipts (warn during grace)
- **orphaned** — receipt reference resolves to no known record (warn during grace)
- **cross-wave** — receipt wave differs from claim wave (always warn)

Pure function over facts + known-id sets, matching the existing `Check`
discipline. The gathering code in `doctor::run` collects known ids from
the store (run_ids, trace turn ids, pr numbers) and wave journals (chat
turn ids, pm ids, memory facts).

### Store queries added

- `SqliteStore::agent_turn(id)` — direct lookup by trace turn UUID
- `SqliteStore::all_task_prs()` — list all Task PRs for `pr:` resolution

### `Receipt::parse_pr_number` 

Moved to the receipt type module (`receipt.rs`) so both the command
resolver and the doctor sweep share one implementation.

## Done-whens satisfied

1. `lf receipt show chat_turn:<turn_id>` opens the exact journaled turn
2. `lf receipt show` works across all five `EvidenceKind`s
3. `lf doctor --json` includes a `receipts` check with missing/orphaned/cross-wave
4. Legacy unsourced facts show as `warn`, not `fail`
5. Unresolvable references exit non-zero with a reason — no partial spoof

## Tests

- `receipt::tests` — parse, round-trip, pr_number extraction (5 tests)
- `lf::commands::receipt::tests` — chat_turn resolver finds journaled turn,
  missing turn errors, worker_report resolver finds run + summary, missing
  run errors (4 tests)
- `lf::commands::doctor::tests` — receipt sweep: no facts ok, resolving
  receipts ok, missing warns, orphaned warns, cross-wave warns, pr resolves
  by number, pr with unknown number is orphaned (7 tests)

## Serial PR plan (from design doc)

- **PR1** (merged #919) — Receipt contract + memory-fact receipts
- **This PR** — `lf receipt show` resolver + doctor memory sweep
- **Next** — Report + KR/Project claim receipts (`ClaimCited`, `lf pm cite`,
  `lf memory cite`, PM snapshot overlay)
- **Then** — Shared render affordance (Mac/iOS/chat)
