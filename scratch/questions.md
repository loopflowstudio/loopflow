# Open implementation notes

- This pass takes the first executable cutover slice: dynamic same-Turn Send
  outcomes, portable Steer fallback, and trace vocabulary. The Work/Epoch/Run
  persistence migration remains a later checkpoint; adding a second dormant
  runtime beside Sessions would violate the no-dual-architecture rule.
- Project and Task still persist live delivery through `ChildCommand` until the
  Steer/Send migration. The controller now keeps every live, rejected, failed,
  or unknown Steer in the next-boundary seed, but crash-proof incorporation
  still depends on replacing that ledger with immutable Send plus Basis.
