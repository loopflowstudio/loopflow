# Make PR waiting explicit

## Problem

The review types removed in Slice 1 survived as behavior. Every open Task PR
bars supervisor recovery, `OpenPrDisposition::AwaitingReview` stops the body,
and `NextMoveOwner::Review` claims user attention. `PrPublication` cannot tell
the difference between `lf pr publish`, `lf pr submit`, and `lf pr land`, so a
mere evidence publication creates an implicit human gate.

Closed PR #1052 carried four useful invariants but tied them to mandatory
`InteractionReview`: publication is not a shipping decision; a shipping
request names the exact head and complete-versus-next disposition; intent is
durable before the GitHub mutation; GitHub executes and reports settlement.
Retain those invariants without restoring review/approval state.

## Design

Represent the one missing real concept on `PrPublication`:

```rust
enum PrMergeMode {
    User,
    Auto,
}

struct PrMergeRequest {
    mode: PrMergeMode,
    requested_at: OffsetDateTime,
    head_sha: String,
}

struct PrPublication {
    // existing publication and after-merge facts
    merge: Option<PrMergeRequest>,
}
```

`None` means published evidence with no merge wait. `User` is written only by
explicit `submit`; `Auto` is written only by explicit `land`. Both are genuine
operator choices and may bar ordinary supervisor restart while the exact head
settles. `publish` and `pr open` do not write a merge request merely because
they create or present a PR.

Persist the request, including the current GitHub head, before assigning or
arming auto-merge. A changed head makes the request stale rather than silently
transferring it. Do not add approval, review, settlement-armed, or generic
blocked state. The existing observed merge remains settlement truth.

Delete `NextMoveOwner::Review` and `OpenPrDisposition::AwaitingReview`.
Presentation derives from facts:

- published/no request: Task flow and Feedback determine the next owner;
- `User`: User owns the explicitly requested merge click;
- `Auto`: CI or GitHub owns mechanical settlement;
- failing CI: the existing typed CI repair path owns the next move.

Use migration `0.12.004`; `0.12.003` is reserved for the concurrent durable
Wave-promotion occurrence repair. Historical publications migrate with no merge
request because their intent cannot be inferred honestly.

## Done when

- `lf pr publish` leaves `merge == None`, an open passing PR creates no review
  owner or supervisor bar, and the authored Task flow can continue.
- `lf pr open` presents the PR without creating a merge request.
- `lf pr submit` durably writes `User` plus the exact head before assignment;
  status names the User merge wait and automatic restart is barred because the
  user explicitly chose it.
- `lf pr land` durably writes `Auto` plus the exact head before auto-merge;
  status names CI/GitHub settlement and ordinary restart is barred because the
  operator explicitly chose it.
- A new head cannot inherit either old request. CI failure still uses the
  existing current-head incident and repair path.
- `AfterMerge` remains only `ContinueTask | CompleteTask`; merge observation
  alone performs the selected continuation or completion.
- Rust/Swift DTOs and fixtures mirror the optional request exactly; no DTO
  default or compatibility reader is introduced.
- Exact searches find no `NextMoveOwner::Review`, `AwaitingReview`, implicit
  open-PR review copy, or automatic user attention derived only from PR phase.
- Focused publish/open/submit/land, supervisor, status, migration, DTO, format,
  and Clippy proofs pass.

