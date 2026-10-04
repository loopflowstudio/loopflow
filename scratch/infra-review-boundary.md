# Infrastructure PR review

Jack Heart requested infrastructure delivery reviews use `pr-review` instead of
`demo`. Existing saved Task Flows stay unchanged; Jack will redirect their reviews.

`.lf/flows/code-review.yaml` preserves feature's design review, implementation
loop, publication, feedback loop, gate and landing. The delivery review remains
`human: true` and uses the PR walkthrough skill. Producing HTML does not complete
the review; Session completion and the following loop-decide retain navigation.

Remaining activation: publish and land the authored Flow, then change the current
infrastructure Project's default Flow to `code-review`, preserving KRs and targets.
Do not select it in the shared plan before fresh Task checkouts can resolve it.

Check: `lf flow show` expands both Flows identically except demo → pr-review; `git diff --check` passes.
