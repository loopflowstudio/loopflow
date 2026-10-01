# Context allocation: remaining questions

2026-09-30. [Current design](audit-context-allocation-and-subwave.md).
[Review decisions](child-memory-discovery.md). Updated against main `de074a2eb`.

## Review resolved

Jack approved retaining main's labelled head-and-tail excerpts when combined
memory exceeds its budget, with complete source access and accurate scope
labels. Keep current budget defaults. No further review choice is pending;
the integration and evidence work below remains for implementation.

## Accepted direction

Address-derived ancestors; memory-only scopes; filesystem discovery without an
injected index; skill-directed immediate-child reads and selected-scope curation;
known duplicate removal; scratch behavior owned by existing budgeting work.
Do not restore Run ownership. Current Session capture retains context payloads.

## Implementation details and evidence gaps

- Use the existing 8k memory / 16k scratch / 16k launch message / 64k assembled
  input budgets as main's current defaults. Jack has not chosen new values.
- The 100k concern Jack reported is not a measured cutoff. Current launch
  accounting excludes native provider context and later reads; observe those
  separately where possible, with unknown coverage explicit.
- Preserve source ownership when excerpts split scope blocks. Check embedded
  Wave seed memory against the same aggregate allowance.
- Reconcile with LOO-329/330's current designs; earlier draft alignment is not
  current agreement. Memory readability does not grant operational Work identity.
- Historical audit: unexplained 600-record window, uncertain harness labels,
  null sampled costs, heuristic rereads, and no joined terminal outcomes.
  The table's 530 operating-instruction Runs exceeds its 525-Run population;
  validate its denominator and repository filter before any quantitative reuse.
- A current Session-history audit and live discovery/curation proof have not
  been run. The old audit scripts do not establish current behavior.
