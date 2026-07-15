# W2-123 review — shared visible-state attention

## What was implemented

Rust now owns a required `TaskAttentionSnapshot` on the shared status and
roadmap Task DTO. It projects green, red, black, or unknown together with one
exact reason, observation freshness, safe lifecycle controls, and the process
and local-progress evidence behind the result.

The Mac app consumes that projection directly. NOW membership follows the
shared level, red rows are grouped by the shared next owner, Roadmap and NOW
render the shared reason, lifecycle buttons come from shared controls, and the
same reason reaches accessibility labels. `lf roadmap` prints that exact
reason as well.

## Key choices

- Rust owns the attention fold. Swift does not reconstruct colors or lifecycle
  legality from Session and process fields.
- Missing evidence is `unknown`, never an implied clean or black state.
- PM completion stays orthogonal to attention. Completed work can still be red
  when unsettled local progress remains.
- The roadmap keeps one machine-wide tmux snapshot. Git work is bounded to one
  cleanliness probe per Task with a durable Session and one additional HEAD
  read when an active PR base exists.
- DTO fields are required in both languages and pinned by shared fixtures; no
  compatibility defaults hide contract drift.

## How it fits together

The status/roadmap read joins PM state, durable Task Session and PR state, the
shared tmux liveness snapshot, and bounded worktree evidence into one Rust
attention projection. Both terminal rendering and JSON use that projection;
Swift decodes the same object and only reshapes it for NOW and Roadmap.

## Done-when comparison

- The shared fixture contains all eight required states: live advancing, live
  human wait, dead dirty, dead authored commits, clean backlog, completed,
  stale active intent, and unavailable workspace evidence.
- Rust round-trips the fixture and independently tests the projection table.
- The CLI row test proves the shared attention reason is printed verbatim
  instead of the older independently selected next-move reason.
- Swift decodes the same fixture and proves NOW membership, shared lifecycle
  controls, and accessibility text containing the exact reason and owner.
- `cargo test -p loopflow lf::commands::waves` passed: 20 tests.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo nextest run --all` passed: 1,373 tests, 2 skipped.
- `swift test` passed: 118 tests in 22 suites.
- The full CI-equivalent gate passed Python, website, Swift, e2e, and the
  macOS app/UI-runner build. The first Rust pass exposed one all-target lint in
  a test helper; it was fixed and the complete Rust gate passed afterward.
- Two hosted UI attempts did not initialize because macOS canceled
  UI-automation authentication. This is a host authorization failure, not a
  test failure; no hosted behavior was exercised in either attempt.

## Risks and bottlenecks

- Roadmap latency now includes bounded Git subprocesses for Tasks that have
  durable Sessions. The probe count matches the design boundary, but large
  portfolios should be measured before widening this contract to Projects and
  Waves.
- A human-wait projection is supported and fixture-proven. The newly merged
  interactive-handoff store record is not yet joined into roadmap state, so a
  real opened handoff will not independently create that state until the
  follow-on integration lands.
- The hosted UI behavior still needs a permissioned run because macOS rejected
  the local test runner before launch twice.

## What's not included

- Project, Wave, Run, Home, history, Audit, or Wave Chat attention contracts.
- Project blocker preservation.
- Opening or completing interactive handoffs and joining them to Task
  attention.
- A second heartbeat, watchdog, or W2-135 body-observation model.

## Review findings

The public contract maps directly to the product concept, has one derivation
owner, and exposes failures as evidence rather than flattening them. No
duplicate Swift state machine or compatibility shim remains. The only code
finding was the all-target clippy failure in a test helper; it was fixed before
handoff.
