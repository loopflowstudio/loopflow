# Assumptions — 2026-11-04

The Task brief selects bounded input-store retries while preserving drafts after
unexpected path loss. Historical request attribution is unavailable after the
owners exited. Fault injection proves recovery handling, not the incident's
cause; the moved-WAL counterexample and causal limits remain in
[the plan](loo-725.md#evidence-limits-and-remaining-acceptance).

Keep history-bearing demo roots intact and unmarked until explicit teardown.
This safe assumption preserves Jack Heart's history requirement without claiming
he accepted indefinite retention. Demo/review must choose whether to separate
durable history from disposable runtime and how to settle drafts/requests before
cleanup. LOO-722's `9ad44189` artifact-retention decision does not establish these
roots' eligibility. The [plan](loo-725.md#demo-cleanup-decision--open-2026-11-04)
keeps the marker contract, alternatives and required proof together.

That choice holds Task completion, not source delivery: either option changes only
`demo.md` and root marking, and `auto` has no demo step in which to make it. This
is a reversible headless assumption, not Jack Heart's decision; the
[publication boundary](loo-725.md#demo-cleanup-decision--open-2026-11-04) keeps the
reasoning and what stays open after merge.
