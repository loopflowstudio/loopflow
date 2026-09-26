# Flow iteration tuples — parent correction, 2026-09-25

## Result and remaining boundary

Each authored backward edge now contributes its own traversal count. The shared
projection supplies an ordered tuple for each active nesting level; Feature can
show `Iteration (2, 1)`, and a selected nested path can show `(2, 1) / (3)`.
Every nesting level remains represented, including an empty root tuple. Preview
has no invented execution count. Each loop region still uses its own shared
`FlowReturn.traversals`.

Removed `FlowReturn.iteration` and the Run/Session presentation scalar. Current
Task and standalone Flow Sessions use the same `flow_iterations` projection;
Run capture records that tuple so a prior Session does not acquire the current
Run's counts. Old captures explicitly lack the tuple and display iteration
unavailable. No scalar-to-tuple inference or capture rewrite occurs.

`ExecutionCursor.iteration` remains an internal visit token: boundary keys,
Task human-session identity and stale-decision rejection still require it.
Deleting that token would change execution authority, not simplify display.
The focused stale-boundary checks below prove it remains effective. No execution
transition, writer, extra Run inventory, provider action or planning mutation
was introduced.

The native polish writer owns the updated layout and loop labels. Parent changed
only its tuple header to consume the shared ordered projection. Full configured
app/demo proof, recent Runs/Session detail, membership-chip navigation and delayed
New-session preparation remain open. The old scalar receipts are superseded.

## Proof

- Seven Rust library tests pass: independent `(2, 1)` counts after changing the
  visit token to 99; nested active versus settled counts; exact nested Session
  node plus captured `(5) / (2)`; old capture missingness; human Session identity;
  standalone stale/conflicting recovery and managed stale-decision rejection.
  `/tmp/loo291-parent-tuple-identity.log`.
- Real isolated CLI `task_flow_read_pins_topology_counts_both_returns_and_rejects_a_bad_restart`
  passes with exact tuple and independent edge counts. No live Task or provider
  was changed. `/tmp/loo291-parent-tuple-cli-final.log`.
- Nine shared DTO tests pass (`/tmp/loo291-parent-tuple-dto.log`); the Comments
  reviewer subsequently strengthened its unrelated wire assertion.
- Five Swift tests in four suites pass: required tuple decoding, nested formatting,
  historical missingness, occurrence states, mounted Flow controls and named
  Session drill-down with retained draft/companion PTYs.
  `/tmp/loo291-parent-tuple-swift-final.log`. These are fixture reads with real
  native panes, not configured provider or human composition acceptance.
- `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and
  `git diff --check` pass (`/tmp/loo291-parent-tuple-clippy.log`).

Initial compilation found one remaining scalar in a controller test and an
unqualified `json!` assertion in the CLI test. Both were corrected; the failed
logs remain `/tmp/loo291-parent-tuple-rust.log` and
`/tmp/loo291-parent-tuple-cli.log`. No failure was treated as a passing receipt.

Pre-edit source copies: `/tmp/loo291-parent-tuple-before`. Source/artifact hashes
are in `parent-iteration-tuple-receipt.json`. No checkpoint claimed another
writer's shared changes. Independent compression/review will inspect this
correction with the next Task evidence slice before the final demo build.
