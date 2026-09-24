# Joined release execution and publication recovery

## Remaining work

Keep the review's iterate disposition. This cut addresses joined execution and
publication-stage read-back/reconciliation, and extends caller-preservation proof.
It does not close the Task or approve the full branch. Next work remains:

1. Retain the prerequisite telemetry due identity for every original covered
   release opportunity, including missing/failed historical evidence and current
   recovery links. Resolve the bounded prerequisite retry choice. The observed
   missing `agent_turns` scorecard table and unassigned repair dependency remain.
2. Carry the existing target lock through every mutation child, including Git/PR
   children, and demonstrate actual parent death with a surviving child. Current
   explicit publisher inheritance does not cover all shared command helpers.
3. Give closed unfinished obligation segments explicit continuation/disposition
   without transferring authority across Homes.
4. Complete remaining interruption/caller-preservation cases and then supported
   installation/sync, required UI-host/public smoke, and two adjacent automatic
   executions. No live publication or installed cutover was attempted here.

## Findings that changed the code

The joined fixture initially failed with `'release-run' is a skill — run lf skill
release-run`. `flow show` loaded the authored flow directly, while execution and
cron selection used skill-first discovery. Both now share discovery that gives
repository flows precedence and propagates malformed-flow errors. GOAL.md's
`crons[].flow` must resolve to a Flow; missing flow content cannot silently install
a same-named skill. Direct `cron add` still resolves an explicitly selected target
through the existing skill/flow catalog. No name-specific release exception was
added. Historical installed skill receipts remain unchanged.

Candidate-only publisher reconstruction previously succeeded when the latest DMG
was absent or wrong. Both new counterexamples failed before the repair. Read-back
now checks that endpoint as well as versioned artifacts, website identity, crate
version, and pinned installer smoke. Rust requires every public verification stage
before settlement, not only the artifact/smoke/UI subset.

Scheduled settlement invokes the existing publisher's `reconcile` operation.
Read-only `verify` reports incomplete stages. Reconciliation treats a public 404
as missing, distinguishes unavailable service errors, and checks GitHub's current
latest tag before repairing missing crate/versioned-DMG publication or stale
website/latest-DMG output. It uses the existing upload, crate, and deployment
operations against the exact source and hash-verified public DMG. It never
re-signs artifacts or republishes GitHub assets to recover a local receipt.
A conflicting versioned object or a newer release fails without mutation.

Each completed repair is persisted in the same ArtifactReceipt before the next
operation; atomic replacement now fsyncs its directory. The final public receipt
requires fresh read-back and smoke. A real failing temporary installer proves
that a repaired external upload survives while verified success remains absent.
The existing local target lock remains held through the reconciliation child.
This adds no scheduler, store, liveness registry, or independent publisher owner.

## Focused proof

| Boundary | Observed result | Limit |
|---|---|---|
| Cron → CLI flow → release → history | Five cases pass: published, no-change, telemetry failure, post-publication smoke failure, missing latest-stage proof | Disposable Home/registry/repository; synthetic dates; simulated telemetry and external services |
| Catch-up cardinality | At least three original dues, one attempt, at most one product settlement | Does not observe host sleep or real launchd timing |
| Caller preservation | Same HEAD, branch, byte-identical index, staged/unstaged README, untracked file, and unpublished local commit in all five cases | Does not prove every kill/interruption point |
| Catalog cutover | Declared release flow selected; deletion and malformed content fail | Does not perform installed cron sync |
| Public reconstruction | Candidate-only proof succeeds only with complete public stages and smoke | Network/signing/UI mocked; temporary installer and binaries execute |
| Repair | Missing crate/versioned-DMG and stale website/latest alias repaired; hashes/source retained | No production side effect |
| Rejection | Newer release, immutable conflict, unavailable service, stale read-back, missing UI proof, installer failure cannot produce success | Local counterexamples |

Commands:

- `cargo test -p loopflow --test scheduled_release_tests -- --nocapture` — passed
  (one joined test, five scenarios; final execution 35.13s).
- `cargo test -p loopflow --lib declared_cron_flow_cannot_fall_back_to_builtin_skill`
  — passed.
- `uv run pytest python/tests/test_release_publisher.py -q -k 'public_proof or reconcile or smoke_failure or publisher_prepares or direct_publisher_stage'`
  — 13 passed, 7 deselected.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and Python
  Ruff check/format — passed.

The new Rust fixture first required a real disposable registry, then exposed the
actual catalog failure. It uses the ordinary cron runner and built `lf` binary;
it does not substitute a shell for the release CLI. GitHub and publisher stage
proof remain explicit simulations. History returns no qualifying pair for those
historical synthetic dates. No affected-suite/full-CI gate was run.
