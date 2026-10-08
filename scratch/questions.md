# Remaining choices — LOO-406, 2026-10-08

The [design](explore-loopflow-s-own-store.md) owns scope, evidence and remaining
implementation. Jack Heart selected one stored plan with repository-wide optional
Linear sync, immediate offline saves with pending sync, and Git-like Task prefixes.
These choices need no further approval; ship-decomposed now owns delivery.

- **Conflict precedence is decided.** Jack Heart selected Linear as winner for
  every planning conflict. Without Linear, prefer the host where possible,
  otherwise last-write-wins with try-not-to-clobber and recoverable losing edits.
  LOO-412 owns host selection and ordering. The demonstrated unseen-write race
  remains a protocol limitation, not an unanswered precedence decision. Commit `185de5fbf` replaces
  pending-field conflict overlays and manual state/comment resolution with automatic
  Linear adoption, retaining losing intentions and stopping their delivery.
  Execution state never enters Linear or machine planning sync; incoming planning
  cannot move Workflows, signal Processes or clean checkouts.
- **Delivery shape changed.** Jack invoked ship-decomposed, then authorized the
  ownership-cut relaunch. `84664e661` is available for LOO-412 source integration;
  compression is checkpointed at `05d36d79f`. Neither establishes shippable connected behavior.
- **Common ownership is implemented for source review.** Wave schema/definitions,
  provisioning, reads/edits/relocation and deletion now use the common local store.
  Source boundary: `84664e661`. Personal types/schema/addresses and provider-first deletion are removed. Existing
  IDs, source files, execution and historical uncertain effects remain. LOO-412 can
  consume the committed boundary; installed migration is unproved. Connected export,
  deletion/cancellation/field delivery, pending
  presentation and composed reconnect remain substantial work. The cut is not yet
  an independently shippable connected product; decomposition cannot restore rejected
  owners or temporary adapters to conceal that gap.
- **Creation identity is known; delivery is unfinished.** Linear accepts supplied
  UUIDs for comments, issues and Projects. Comments use exact identity/issue/body
  readback; Task/Project export still needs proof. Uncertainty never permits a new
  identity. Semi-live completion, comments and independent membership acquisition
  remain required through pending writes and reconnect, without echoes or replay.
- **User-keyed planning is the default.** Jack selected explicit shared-plan opt-in
  with the same model/APIs. LOO-412 owns stable user identity, destination binding,
  safe joining and shared conflict ordering; no implicit sharing by code remote.
- **LOO-412 owns transport.** Jack selected the custom Git planning ref prototype
  (`8b45e82d-6765-490a-a3d6-44b44611cedd`). Isolated protocol fixtures can proceed;
  integration consumes LOO-406's coherent committed writer, never dirty code or
  duplicate planning. No machine replication here or public code-repo plan
  publication is authorized. Incoming planning grants no Workflow movement,
  Process control or checkout cleanup. Privacy, laptop-loss recovery and callbacks
  remain outside this PR; the design pins their earlier evidence.

October 8 conflict reconciliation queried `lf context --skill implement`: authored
memory and scratch fit; the generated Work seed remains 18,382/16,000 goal tokens
(2,382 over). Stored steers and budgets remain unchanged. The generated seed cannot
be shortened by editing its cached copy; removing accepted direction is not selected.
