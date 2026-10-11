# Session title hook timeout

Jack Heart requested an autonomous repair on 2026-10-10, through publication
for review; no landing. No prior design was present in this checkout.

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

Gate owns post-integration affected suites. Acceptance still needs the installed
hook returning inside the unchanged five-second deadline under concurrent Flows;
four simultaneous callbacks against held locks are the existing headless proof,
not an observation of that configured workload. Installation acceptance belongs
to a capable gate/release environment, without promoting a branch binary into
the live store. Publication belongs to the caller's delivery step; Jack Heart's
review remains required before landing. No new product decision is needed for
this bounded repair; correlation of the records-read report remains unknown.

Checks: prior to integration, `cargo test -p loopflow --test session_cli_tests native_title_callback` (4), `cargo test -p loopflow --lib journal::tests` (18), `cargo fmt --check`, and `cargo clippy --all-targets -- -D warnings` passed with inherited `LF_*` cleared; `harness::native_titles::tests` (2) retains its earlier pass; reconciliation's `git diff --check` passed and `lf context` fits both budgets; post-integration suites/installed acceptance remain with gate/CI.
