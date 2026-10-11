# Project acquisition dependency — LOO-412

2026-10-10 · Operator review of `71bafc5ada` against the accepted
[reduction](work-model-reduction.md) and [current design](work-on-another-machine-name.md).

The prose round trip remains absent after the typed-field, Markdown and save-path
checkpoints. Those are useful components, not the coherent acquisition outcome.
`ProjectNode::into_pm_project`, `sqlite/planning.rs::put_pm_project` and
`project_accepted_planning` still require `PmProject`; receipt baselines read its
serialized body. Adding only a prose edit cannot supply the missing observation.

The queue's “existing passes first” wording must not defer removal of this direct
dependency until after acquisition. The accepted Project reduction already calls
for a canonical Project and a concrete private Linear observation. That change
belongs in the acquisition cut, together with all affected consumers and retained
observation evidence. It does not require Jack Heart to choose another model or
approve an exact edit shape. Unrelated later audit groups retain their order.

Completion evidence is an ordinary Project prose save followed through selected
Linear delivery and acquisition into the same canonical Project, with receipt
settlement. Exact values must survive boundary whitespace consistently; omission,
empty content, pending successors and conflicting observations retain their distinct
meanings. Parser-only tests and another internal helper checkpoint cannot establish
that outcome. Existing migration bytes and unknown effects remain preserved.

This finding changes dependency ordering within the accepted scope; it authorizes
no second writer, Flow restart, publication or installation. Remove this note once
the current design absorbs the dependency and the composed behavior has proof.

In-flight review: moving `PmProject` to `pm::linear::LinearProject` does not by
itself remove the second domain view. `plan_read.rs::project_in` still constructs
that type from local Work, substitutes external/local IDs with `COALESCE`, and sets
description to unavailable even for a saved local Project. `ResolvedTask.project`
and `PmSnapshot.projects` still expose it to ordinary callers. These are concrete
remaining consumers, not evidence of an adapter-only observation. The final cut
must remove those uses; a symbol rename must not mark audit sections 1–3 complete.

Also check full observation → partial observation → local prose edit. Current
`put_project` ignores omitted description for comparison but replaces the entire
stored body, losing the prior prose observation. `record` then has no baseline.
Retain its original revision/age under a concrete owner; copying old prose into
a newer observation would instead fabricate freshness. Canonical value retention
alone does not prove observation/baseline preservation.
