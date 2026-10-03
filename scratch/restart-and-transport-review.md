# Restart and transport review — 2026-10-02

Status: interactive design review; cached-planning policy awaits Jack Heart's
answer. No new acceptance or implementation authorization is recorded.

The [working design](make-the-release-and-ci.md) retains all four outcomes from
LOO-326. Jack's supplied steers establish the Swift repair's shipment; the
[startup diagnosis](loo-326-worker-startup.md) remains separate historical
evidence and does not authorize a live restart.

Source review confirms remote planning acquisition at restart, continuation and
managed Flow admission. The proposed local-read policy must cover all three.
The question presented to Jack is whether otherwise valid cached facts admit
existing work regardless of age. Known invalidation, removal, terminal state and
ownership mismatch still block; publication and completion retain their checks.
New advice still requires Linear publication before worker replacement.

Design refinements from source review:

- Bound output draining as well as child exit, including descendants retaining
  pipes. A locally enforced timeout consumes one of the three read attempts.
- Advice-publication failure preserves the old worker and Flow, but the existing
  ordering may already have checkpointed the checkout and updated cached Project
  data. Do not promise rollback or a byte-identical checkout.
- Run the complete Swift transport suite through the existing headless Desktop
  wrapper. Its sandbox denies WindowServer only; external network denial still
  needs its own proof, including descendants and usable loopback.

Next useful action: capture Jack's cached-planning decision in the design and
[open questions](questions.md). Implementation and behavioral verification remain
outstanding. No Flow navigation decision is made here.

Check: inspected task/release/check-reader code and Desktop test wrapper; only
design notes changed, so behavioral suites were not run.
