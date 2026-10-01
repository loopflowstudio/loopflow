# Context allocation: remaining evidence

2026-09-30. [Current design](audit-context-allocation-and-subwave.md).

Jack Heart's latest steer assigns ancestry to LOO-354, budget mechanics to
LOO-356, and the planned 16,000-token default/gradual curation to LOO-362.
LOO-331 implements child-memory curation guidance and duplicate delivery removal.
PR #1375 was OPEN at implementation start; this patch does not depend on it.
No further review decision is pending.

- Demo/review still needs observed child discovery and useful parent curation.
  Authored instructions and prompt fixtures establish delivery, not compliance.
- Current Session-history allocation measurements remain unavailable. Historical
  Run counts, provider labels, null costs, heuristic rereads and unjoined outcomes
  must not be presented as current results. The historical operating-instruction
  count of 530 exceeds the 525-Run population; resolve it before quantitative reuse.
- Jack's reported 100k concern is not a measured model-quality threshold. Launch
  budgets exclude native provider context and later reads; no new limit is chosen.
- Implementation choice: deduplicate only the same canonical file with the same
  bytes, plus an exact builtin LOOPFLOW.md when operating guidance is enabled.
  Equal text from distinct memory paths and customized operating instructions stay.
