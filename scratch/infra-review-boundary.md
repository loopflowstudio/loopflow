# One pursuit, explicit reviews

Jack Heart requested one pursuit without a demo or outer return loop. The earlier
separate infrastructure Flow is superseded: `pursue` loops through implementation,
compress, refresh and loop-decide, then publishes; `code` adds human `pr-review`;
`feature` keeps design review, then pursuit, demo, queue and landing.

Existing saved Flows remain unchanged. Jack will redirect existing reviews.
Remaining activation: publish and land the definitions, release/install them,
then set the infrastructure Project default to `code`, preserving its KRs and
targets. No shared default should select the old code semantics prematurely.

Checks: `cargo test -p loopflow --lib engine::flow` — 55 passed; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` passed.

Sync check: `cargo test -p loopflow --lib engine::flow_graph::tests` — 10 passed after merging main; review boundaries retained with current command names.
