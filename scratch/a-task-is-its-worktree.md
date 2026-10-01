# A Task is its worktree's work — LOO-358

Accepted direction: Jack Heart, 2026-09-30. Task status, navigation, recovery
and completion observe every AgentSession, FlowSession and Exec associated with
the checkout, plus explicit binds. The managed Flow is a marked member.

## Implementation decisions (2026-09-30)

The base contains LOO-298's three owners but no shared LOO-353 association reader.
Put association in Rust's SQLite reader and share it with inventory filters and
Desktop. Include checkout descendants with component boundaries, retain explicit
bindings outside the checkout, and never infer membership from causal ancestry.
Association is observational; it does not rewrite usage attribution or grant
signaling, driver, settlement or worker authority. Retained paths work even when
the checkout is missing. A missing path never matches everything.

## Delete — do not maintain

- Removed `TaskSnapshot.runs` / `runs_truncated` and misleading Run labels.
- Removed Desktop's separate Task-binding lookup; use Rust `task_ids`. Keep detailed
  conversation history alongside the general Sessions, Flows and Execs inventory.
- Managed-only cleanup/abandonment checks; inspect all associated execution.
- Task-filtered Session and Flow inventories restricted to explicit Task IDs.

Preserve recorded histories, managed cursor/claim fencing, precise provider
completion, explicit binding, and prospective usage ownership. No schema migration
or alternate durable owner is required.

## Managed selection retained

- Task run/retry/restart/stop advances and controls one worker's captured Flow.
- Exact Task review settlement and Task delivery authority fence that worker.
- The managed Flow panel describes that cursor; general inventories describe work.
- Completion may settle the managed Flow after delivery; independent Flows retain
  their own completion authority and must not be silently ended.

## Review findings resolved

- Desktop's binding-only grouping would still orphan checkout-only Sessions;
  Session metadata now carries the same Rust association as Task status.
- Recovery must preserve idle independent Flows, while completion waits for them.
  Recovery checks unresolved execution; completion also checks unfinished Flows.
- Managed is relative to the inspected Task. A Flow managed by another Task
  cannot inherit this Task's completion exemption through checkout association.
- Keep detailed conversation history alongside the general inventory: it exposes
  provider outcomes that the owner list deliberately does not decode.
- Compression removed Desktop's duplicate membership rule; explicit bindings now
  appear in fixture `task_ids`, matching Rust's additive association. Task Session
  rows decode directly without temporary placeholder kinds, and managed Exec
  exclusions use set membership rather than scanning history for every Exec.

## Remaining verification

Gate owns affected suites and broader lifecycle acceptance. Configured Desktop
judgment remains with demo/review; no publication or installation was attempted.

Checks: `cargo test -p loopflow --lib task_work_` 2 passed; `cargo test -p loopflow --test dto_fixtures` 13 passed; `swift test --package-path swift --filter 'DTOFixtureTests|WorkspaceNavigationTests'` 45 passed; `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` and `git diff --check` passed; broader acceptance → gate.
