# Session title hook timeout

Jack Heart requested an autonomous repair on 2026-10-10, through publication
for review. His later comment `d6ad11df-fd8b-4b4a-861b-ac7ab785cb26`
requests landing PR #1530 after synchronizing and resolving conflicts in place.
No prior design was present in this checkout.

## Implemented design (implementation decision, 2026-10-11)

Dispatch `lf __session-title <provider>` before ordinary command admission.
Retain best-effort Process recording through the existing noninitializing
observer, with a 100 ms receipt wait instead of 15 seconds. Do not repeat a
failed early observation at exit. Reuse its connection for parent validation.
Preserve installation routing, custom names, managed-session ownership and
historical provider identity. Bound ownership reads to 100 ms lock waiting and
100 ms SQLite execution; unavailable ownership leaves the name unchanged.
Codex retains its three-second RPC deadline; Claude calls no provider or Linear.
No migration, worker, retry queue, cache or increased provider timeout.

The callback no longer enters ordinary admission/account selection or names
through `execute_command`. Its dedicated bounded reader retains the shared
historical ownership query; other consumers retain `open_processes_read_only`.

## Evidence and boundaries

Reproduction base: `21dae527b4da14b57d96286f80e3fb3cec09eaa3`. Main journals every command;
ordinary admission can spend 15 seconds on its first receipt. A disposable WAL
writer held the baseline callback past 6.006 seconds with `{}` input, before
any title lookup or provider call. The fixture killed only its own child.
The ownership reader independently permits a five-second lock wait and scans
observation history; the repair bounds both.

After repair, isolated CLI probes took 29 ms without a database, 154 ms with a
held WAL writer and 296 ms with an exclusive rollback-journal lock. These are
local samples, not installed-fleet measurements. Four simultaneous callbacks
are covered under both locks. A lazy billion-row history fixture proves scans
are interrupted without renaming. Missing-store naming creates no database;
uncontended callbacks still record completed Processes. Custom and managed
names retain their existing tests. Two initial post-build runs exceeded a two-second fixture limit with no output;
that startup delay was not attributed. Direct warm probes were below 300 ms.
The retained four-second fixture ceiling includes startup/scheduling headroom
below the provider's unchanged five-second timeout.

The reported four-minute Wave status and 110-second Session ensure share
ordinary admission, but their full cause is not established by this reproduction.
The unattributed “could no longer read its own records” report has no captured
error or trace; a common cause remains unresolved. Missing historical checkout
Git errors from task reconcile do not establish SQLite contention. No broader
status repair is justified by this evidence alone.

Jack Heart reported on 2026-10-11 at about 00:11 PDT that two consecutive
prompts in a Claude Session in Kata (a branch building on KAT-32) showed the
same five-second timeout, with one background shell still running (comment
`d4991d61-da39-4b7a-af0f-15880c234d82`). The incident is neither specific to
the Loopflow repository nor dependent on this Wave's concurrent Flows.
Ordinary admission reaches the shared Machine store before inspecting the
hook's repository or request, so the reproduced wait can affect other
repositories and repeat while contention persists. The report supplies no
lock trace: the background shell is context, not an identified lock holder,
and the Kata incident's exact cause remains unconfirmed. Regression coverage
now invokes two consecutive batches of callbacks from a separate disposable
directory while the same writer remains held, under both lock modes.

Review corrected the initial omission of Process history, retained the same
bounded connection for inherited-parent reads, and prevented an unbounded
exit-time observation retry. No parallel naming or persistence path remains.
The observer now owns its one-attempt rule; completion needs no separate flag
check. Its writable schema probe checks the same columns once, and ownership
tests reuse the contention tests' bounded callback helper. No deletion targets
remain in this scope.

Reconciliation on 2026-10-11 inspected merge `058f02eedf`: upstream
`81910e6e4d` adds Process-owned liveness and terminal evidence. Its journal and
Session fixture changes preserve the hook's bounded observer and ownership
reader; no second liveness or naming path is needed. The prior focused passes
predate this integration. Release child memory was read in full; its distinction
between source proof and installed acceptance applies here too.

## Remaining

Post-integration Session CLI, journal and native naming suites pass. Acceptance still needs the installed
hook returning inside the unchanged five-second deadline under concurrent Flows;
four simultaneous callbacks against held locks are the existing headless proof,
not an observation of that configured workload. Installation acceptance belongs
to a capable gate/release environment, without promoting a branch binary into
the live store. Delivery belongs to the caller's delivery step under Jack Heart's
later landing request. No new product decision is needed for
this bounded repair; correlation of the records-read report remains unknown.

Gate review retained the pending fixture correction: each callback's deadline
starts at spawn, including callbacks collected later in a concurrent batch.
The production path preserves one observer and one ownership query; README
describes the bounded fallback. No further production changes were needed.

The earlier interrupted materialized gate log at
`.lf/tmp/gate/run-52817/rust/rust.log` contains two failures and no completion.
The account test `claude_cached_identity_requires_current_verified_credentials`
still fails in isolation at `account_status.rs:677` (Unavailable, expected
Accepted). That file is unchanged from the checkout base; the cause is unresolved
and outside this title-hook design. `task_comments_read_and_publish_without_starting_work`
passes in isolation; that does not establish a fix for its earlier timeout.
The full gate remains unproved; CI must resolve the account failure and complete
the materialized matrix before delivery can claim a green gate.

Checks: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `uv run python scripts/check_architecture.py`, and `git diff --check` passed; network-isolated `cargo test -p loopflow --test session_cli_tests --test global_commands --test documented_commands` passed 30 tests (2 disposable-OS installation tests ignored), `--lib journal::tests` passed 18, `--lib harness::native_titles::tests` passed 2, and `--lib task_comments_read_and_publish_without_starting_work` passed 1; `--lib claude_cached_identity_requires_current_verified_credentials` failed; full materialized verification belongs to CI and installed concurrent-Flow acceptance remains with release.
