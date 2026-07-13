# PR #872 review guide: Wave → Project → Task control

## What was implemented

Loopflow now treats Wave, Project, and Task as the three durable runtime tiers.
Linear remains the planning authority for Projects and Tasks; SQLite joins that
planning snapshot to provider continuity, commands, directives, observations,
process generations, and Task delivery.

- Added durable Project and Task Sessions with stable Linear identity,
  restartable provider history, typed lifecycle state, and audited control
  commands.
- Added versioned child directives. Initial direction is persisted before the
  provider starts; replacement steering advances the version; explicit child
  acknowledgement proves incorporation.
- Added root-Wave authority and material descendant observations without
  copying raw child tool chatter into Wave Chat.
- Added a native cache-only Wave → Project → Task status snapshot and carried
  it through Rust JSON, shared fixtures, Swift models, and the Wave work map.
- Added structured Wave Chat activity for direction, incorporation, decisions,
  blockers, delivery, merge, and abandonment while retaining one human
  composer.
- Moved Wave and Project provider turns to the clean canonical `main` checkout.
  Only a Task owns a worktree, branch, or PR. Wave-level diff, branch, PR, and
  land projections and endpoints are gone.
- Removed the competing generic detached-loop, queue, stack, `lfq`, `combine`,
  and `next` product surfaces. Wave, Project, and Task each execute their
  clarify/pursue/mutate policy flow; deterministic controllers own lifecycle
  transitions.

The gate also fixed failures found while reviewing the branch: Project commands
could replay every poll; stale superseded input could still be delivered;
attachment input bypassed directive versioning; pre-directive sessions lacked a
forward migration; stale runner writes could roll directive versions backward;
unsupported Wave steering did not restart the interrupted phase; Task
no-progress detection missed edits to existing untracked files; and nested
runtime launches could inherit the parent's Project/Task identity.

## Key choices

### Planning truth and runtime truth stay separate

Linear owns Project definitions, KRs, and Tasks. SQLite owns execution and
control. `lf status --json` joins the cache-only PM snapshot to runtime rows; it
does not synthesize missing Projects or ask Linear during a status read. Drift
fails with an explicit `lf pm sync` instruction.

### Directives are stronger than messages

`follow-up` adds context without changing intent. `steer` and interrupt with a
replacement create a new monotonic directive version in the same transaction as
the command. Provider acceptance proves application; only
`project/task acknowledge` proves semantic incorporation. This preserves honest
receipts across provider capability differences and process restarts.

### Authority is rooted at the Wave; supervision stays local

A Project normally supervises its Tasks, but the owning Wave can inspect and
override every descendant. Material outcomes route to the Wave independently
of immediate Project wakeups. Foreign Waves and unrelated Projects fail before
command persistence.

### The control plane does not ship code

Wave and Project turns use canonical clean `main`; Task Sessions alone own
immutable sibling worktrees and PRs to `main`. Runtime shells clear inherited
Wave/Project/Task identity before exporting the child session's own identity.
The old Wave delivery projection was deleted rather than retained as an adapter.

### No generic session product was extracted

Project and Task share storage and provider-control mechanics where proved, but
remain domain runtimes with different lifecycle policies. Provider sessions,
process generations, and command rows stay implementation details rather than a
fourth public planning noun.

## How it fits together

```text
Human ↔ Wave Chat
          │
          ├── Linear Initiative → Project → Task       planning truth
          │
          └── Wave root authority
                └── Project Session                    clean main
                      └── Task Session                  one worktree + PR

SQLite: sessions + commands + directives + events + observation outbox
                      │
                      └── lf status --json
                            └── Swift Wave work map + linked activity cards
```

The Project and Task runners reserve a process generation, resume the same
provider transcript when supported, settle commands atomically at turn
boundaries, and sleep without spending provider turns when external state owns
the next move. GitHub merge observation completes the same Task identity and
reconciles Linear afterward.

## Risks and bottlenecks

- **Review size:** this is 178 files and roughly 17.8k additions / 9.8k
  deletions across 48 commits. The useful review order is: migrations and store
  invariants; Task/Project runners; `lf status` DTOs; Wave observation folding;
  Swift work map; deleted generic surfaces.
- **Canonical-main coordination:** Wave and Project turns are read-only by
  contract and are admitted only on clean `main`. A Project rechecks after each
  provider turn and fails loudly if the checkout or branch changed. There is no
  checkout lease or sandbox, and the Wave resident currently performs its hard
  check at startup rather than after every phase; an errant tool can still dirty
  main and require manual cleanup.
- **Standing-frontier Projects:** an unchanged fingerprint with open KRs becomes
  `Blocked`. A healthy indefinite frontier may eventually need a distinct wait
  policy instead of no-progress blocking.
- **Provider breadth:** focused capability tests prove honest Codex live-steer
  versus Claude/OpenCode replacement behavior, but the planned ten-scenario ×
  three-provider crash/decision/observation conformance harness is not present.
- **UI execution:** deterministic Swift tests and the macOS UI target compile.
  The known headless `LoopflowUITests-Runner` connection hang remains unproven,
  so no interactive UI run or screenshot is evidence for this gate.
- **External dogfood:** the live two-Task Linear/GitHub path would create
  records, worktrees, pushes, and PRs. This headless gate intentionally did not
  perform those side effects.

## What's not included

- Task-internal Workers and worker scheduling. The status snapshot does not
  publish a fake zero-worker summary.
- Provider approval mapping beyond the existing durable decision protocol.
- The scripted Task → Project → Wave → human conformance scenario.
- Remote Project/Task execution, alternate PR targets, stacked delivery, or a
  generic multi-product runtime.
- A separate Project or Task chat composer; direct CLI controls remain the
  operator escape hatch.
- Rich raw child transcript browsing in the Wave work-map inspector.

## Validation

The final branch state passed:

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --all-targets -- -D warnings` | pass |
| `cargo nextest run --all --no-fail-fast` | 1,315 passed; 3 skipped |
| `uv run pytest python/tests/` | 52 passed |
| website tests through `scripts/test.py` | 59 passed; 3 skipped |
| `swift test --package-path swift -Xswiftc -gnone` | 297 passed |
| Swift multiplatform boundary check | pass |
| `tests/e2e/test_smoke.sh` | pass |
| macOS `xcodebuild build-for-testing` | `TEST BUILD SUCCEEDED` |
| `uv run python scripts/test.py --rust --python --swift` | 4 suites passed |

The cache-only live read also succeeded:

```bash
target/debug/lf status infrastructure --json \
  | jq '{wave: .wave.name, projects: (.projects|length), tasks: ([.projects[].tasks[]]|length), runs: (.runs|length)}'
```

```json
{"wave":"infrastructure","projects":3,"tasks":13,"runs":2}
```

Static deletion checks found no `lfq`, `/v0/exec`, `combine_prs`, or
`next_wave_handler` references in the active Rust, Swift, README, docs, or test
surfaces.

## Simulated operational review

- **Can it be explained in one screen?** Yes: Linear plans, SQLite controls,
  Wave directs, Project pursues, Task ships.
- **Does the API map to the real thing?** `lf project` and `lf task` name domain
  lifecycles. Provider threads and process attempts are not promoted into public
  planning objects.
- **What breaks at 2 a.m.?** Every failed/blocking runtime state carries a
  reason; command receipts distinguish persistence, application, and
  incorporation; process identity and generation survive relaunch.
- **Is flexibility earning its keep?** Generic loop, queue, stack, Wave land,
  fake Project/Task Runs, and placeholder worker projection were removed. The
  remaining common child-control layer has two real consumers.
- **Would deleting code make the system more true?** This pass deleted the
  Wave-level delivery model and stale keyboard action after Task became the sole
  delivery owner. Further deletion should wait for live dogfood rather than
  guessing away recovery paths.

