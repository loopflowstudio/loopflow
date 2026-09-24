# Task Watch gate — 2026-09-23

Disposition: **not ready to ship**. The branch supplies a passive inspection
foundation; the accepted Watch experience remains incomplete. The affected-suite
runner stopped at resource preflight, before any product tests or Clippy ran.

## What was implemented

Task position transactions retain immutable expanded plans, exact stage/Run
bindings, attempts, transitions, and settlement in the existing Task ledger.
`lf task watch ISSUE --json` projects those facts with attributed Runs, including
auxiliary Runs and explicit missing evidence. It does not require a worker or
surviving worktree.

`lf task output ISSUE --json` reads Run journals and recorded native Claude,
Codex, and OpenCode sources. Source groups carry labels, availability, records,
revisions, and gaps. Native tool calls/results retain correlation IDs separately
from record identity. File/stdin cursors avoid argument-size limits. Swift exposes
typed reads and matching fixtures; no Watch view is implemented.

## Key choices and architecture

FlowPosition remains execution authority. Inspection receipts are written in its
own transactions, excluded from parent observations, and folded by Rust without
loading current YAML. Provider Session receipts retain launch-owned locations;
passive readers do not choose accounts or control provider clients. Swift decodes
these projections rather than maintaining another flow or transcript authority.

OpenCode continuation freezes each sweep's upper timestamp and retains inclusive
boundary/unfinished hashes. JSONL continuation uses byte offsets and reset
evidence. Both are bounded transcript readers; neither makes discovery bounded.

## Review findings and remaining acceptance

1. **Directory failures still take down healthy output (P2).**
   `run_record.rs::record_dirs` propagates prefix enumeration, metadata, and
   directory-read errors with `?`. Both Task reads call it before manifest-level
   gap handling. An unreadable or disappearing prefix therefore aborts the whole
   request. This branch also changed the older shared scanner from warning and
   continuing to failing, affecting `scan_runs_since`, unresolved Session scans,
   and manifest resolution. Preserve healthy directories and expose uncertainty
   to Task readers. Prove a bad prefix beside a healthy Task Run and subsequent
   recovery. This is a source finding, not a fresh runtime reproduction; repair
   and behavioral validation remain before shipping.
2. **The full Done When fails by source inspection.** TaskWorkspaceSection still
   contains only Changes and Terminal. Watch navigation, connected selectable
   stages, filters, Follow live, stale state, hidden-view cancellation, and
   accessibility behavior are absent. There is no UI to capture in this pass.
3. **One-second polling is not ready.** Discovery scans all Run manifests and
   retained Task facts per request. Watch is unpaged. Output has one continuation,
   with no independent history/live cursors; retained per-Run and mutable-boundary
   state can reach the 4 MiB cap. The eight-source round robin bounds source
   reads, not total discovery work or time to observe every Run.
4. **Capture and configured proof remain incomplete.** Summary-only journals
   report limited capture; complete tool results and provider attribution across
   attempts remain requirements. Earlier configured Codex arrival evidence does
   not prove Claude/OpenCode live arrival. Arbitrary backdated/equal-count
   OpenCode rewrites remain unproved. No new configured CLI or provider proof was
   run here, and the complete autonomous/native/Iterate/reopen demo and pinned
   human gate remain outstanding.

These are unmet requirements in the existing Task, not proposed scope reductions.
The shared read ownership supports Product/Desktop's contract, but no Project KR
or complete Watch outcome is established by this gate.

## Validation and evidence boundary

Review began at `00524394c69f565b622f4b8aa904c0da316788f6`, with a clean tree.
Another run subsequently edited the snapshot, store history, Task read helper,
and design in this shared checkout. Those edits were preserved and are not claimed
as this gate's implementation. This gate removed duplicate README text, clarified
its manifest-gap and unfinished-interface claims, and ran the standard formatter.
It made no behavioral changes and staged or committed nothing.

Selected with `uv run python scripts/test.py --list`: architecture, Rust,
website (architecture documentation changed), and Swift. CI commands match
`cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`.
Python had no mapped changes; slow CLI/UI suites and hosted UI execution remain
separate checks. The full local matrix was not selected.

| Check | Fresh result |
| --- | --- |
| `env -u LF_RUN_ID uv run python scripts/test.py --reuse-passing` | Resource preflight failed; no product suite ran and no prior pass was reused |
| `uv run python scripts/resource_envelope.py --recover` | Still failed: active main build 19.1 GiB / 12 GiB; recovery cannot remove it |
| `uv run python scripts/check_architecture.py` | Passed; zero unexplained owners/mirrors/shims |
| `uv run python scripts/check_swift_multiplatform_boundaries.py` | Passed |
| `cargo fmt --all -- --check` | Initial failure in concurrently edited Task helper; `cargo fmt --all` then check passed |
| `git diff --check main` | Passed again after README and handoff artifacts were written |
| Clippy, Rust tests, website suite, Swift suite | Not run; resource envelope remained unresolved |
| Full Done When / configured Watch demo | Not met; UI absent and capture/continuation work remains |

Resource measurement: 200.0 GiB free against the 64 GiB floor; this checkout's
build was 4.9 GiB. The blocker is the active main checkout's per-build limit, not
free space. Recovery was attempted once; no active build was deleted and no
budget was relaxed. Logs for this pass: `/tmp/loo-293-gate.log` and
`/tmp/loo-293-resources.log`. These local diagnostics are not durable pass receipts.

Earlier focused passes remain documented in the design and historical reviews;
they are not an exact-tree gate pass for the concurrent snapshot changes. Once
the active build owner resolves pressure and the tree settles, rerun the selected
runner. Reconcile any new findings before publication. The prepared PR copy is a
review handoff, not approval to land or complete the Task.

Final formatting check also passed at the same HEAD. Concurrent edits continued
through the synthetic CLI proof script; no test result from that other run is
included here. Handoff reference is the current HEAD, not a fingerprint of the
still-changing uncommitted implementation.

## Risks, bottlenecks, and intentional exclusions

The remaining blockers above are in scope. Multi-Task dashboards, graph editing,
and expanded remote transport are deliberately outside this PR. The read path
adds no migration, second transcript store, provider lifecycle control, or Swift
execution authority. Live performance coverage is absent: before adding polling,
measure poll latency, bytes read, discovery size, continuation size, and time to
observe new output on realistic retained history; local timings alone do not
prove production behavior.
