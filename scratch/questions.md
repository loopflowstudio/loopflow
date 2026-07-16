# Context Lab review notes

- Context Lab's visible population is Wave-scoped. Project and Task do not
  appear as population filters.
- Context Lab is entered from the selected Wave header. The global Go-menu
  entry, repo editor, Wave picker, and choose-a-Wave empty state are gone; saved
  research views are keyed by repo and Wave.
- Every Task still belongs to a Project. A single-Project Wave routes
  automatically; a multi-Project Wave stores one explicit Refinement Project
  preference without changing the evidence query.
- **Refine in task-worker** refreshes the Wave plan, re-reads Context Lab,
  verifies main's exact source hash, creates `Refine <source> <hash>` through
  `lf task start`, finds the resulting Task receipt, and opens its Agent view.
- The branch has not created a real Task merely to test the button. That
  external write is the first checkpoint in the human review journey.
- The branch is rebased onto current main and uses its converged migration
  chain through `0.11.012_provider_account_lifecycle`; Context Lab adds no
  competing migration tail.
- The current product-Wave snapshot shows `headless surface` at 50 impressions
  and `wave_pursue` at 18. Selecting `headless surface` opens main's current
  `headless.md`, its revision evidence, representative sessions, the remembered
  Refinement Project control, and the guarded handoff.

## Human review checkpoint

Choose `product` → **Context Lab** → **Sources** → `headless surface`, select
the intended Refinement Project, and click **Refine in task-worker**. Confirm that a new Task
opens on its running Agent view with the source path, current hash, Wave query,
measurements, and trace addresses. Review the resulting source diff and use the
Context Lab backlink before deciding whether to keep or cancel that Task.
