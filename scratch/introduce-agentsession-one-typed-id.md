# LOO-442: provider conversation identity

Jack Heart requested AgentSession on 2026-10-09, stacked on LOO-441 / PR #1516.
The Task brief is accepted scope; the implementation selects its permitted
opaque typed-id path, without a table or migration. Jack's parallel-work steer
leaves engine/driver columns and Process ownership exclusively with LOO-443.
Jack's October 9 comment `901bf763-b15d-4940-a6b3-1b2967f5ed90` approves
landing PR #1517 after syncing onto main, superseding publication-only direction.
`lf sync` integrated merged parent #1516 (`461577746`) without conflicts as
`adf3f9e4b`. Gate does not perform the later delivery operation.

## Delete — do not maintain

- Bare-string `provider_thread` / `provider_session_id` code fields, parameters
  and accessors: replaced end to end by `agent_session: AgentSessionId`.
- Codex's duplicate getter and OpenCode's intermediate string parser: removed;
  retain opaque provider bytes, resume behavior and account attribution.
- OpenCode's separate resume/create helpers and optional success result are
  replaced by `open_agent_session`: create only without saved identity, and
  share startup failure cleanup. A failed saved-id lookup never creates a
  replacement conversation. Swift uses RawRepresentable's string Codable
  implementation instead of duplicating its encoder and decoder.
- Keep the SQL/JSON encodings and historical fixtures below, not parallel
  compatibility paths. Engine/driver columns remain outside this Task.

## Implemented boundary

`AgentSessionId` preserves provider-issued strings, including non-UUID ids.
Rust resume configuration, harnesses, native turn keys, capture evidence and
account routing now carry it; Swift mirrors SessionEvent and native history
references. Provider protocol names remain vendor-owned.

The LfSession row holds the current connection; a provider announcing a new
conversation replaces that selection. Immutable turn/capture events retain
earlier native ids and recorded account attribution. Current selection and
per-input history are different facts, not reasons for another table or a new
replacement event. No engine/driver column changed.

## Deliberate persistence boundary (PR notes)

- Keep SQL `provider_thread`: existing Session selection and event keys retain
  their bytes and indexes without a naming-only migration.
- Keep SQL `provider_session_id`: existing provider-account routing keys stay intact.
- Keep JSON `provider_thread` and `provider_session_id`: captured native evidence
  and the Rust/Swift DTO retain one existing encoding, not dual-read aliases.
- Released migration SQL and historical schema fixtures remain immutable.

## Verification and remaining delivery

The merged-parent tree retains opaque ids, original SQL/JSON keys, account
attribution and Swift native-history row ids. Review found no additional code
repair within the typed-id scope. No engine/driver columns or migrations changed.
Fresh-process resume uses two stand-in Claude processes, not a live provider;
source checks establish neither installed acceptance nor live-provider continuity.
Earlier publication/compression evidence remains at
`5cdfae63c:scratch/introduce-agentsession-one-typed-id.md`.

### Remaining gate findings

The affected-suite run completed, including every Rust target despite failures.
The materialized Rust run had five failures; AgentSession resume, opaque-id,
account, replacement-history and DTO cases passed. Three failures in Wave cron
lookup, authored-goal context and exact-Task roadmap pass with canonical
`TMPDIR=/private/tmp`. Their fixtures stored `/var/folders` aliases while readers
resolved canonical repository paths. TESTING.md now carries this gate setup.

Two failures remain, outside the AgentSession implementation:

- `lf::commands::install::artifact_tests::a_settled_install_keeps_only_what_it_can_select_or_is_running`:
  the copied `/bin/sh` produces no readiness line. An isolated probe confirms
  the copy exits on SIGKILL (-9), while `/bin/sh` itself prints `ready` and exits
  zero. The killer/cause is unproved; retention behavior needs a capable host/CI.
- `work_watch::a_checkout_changed_on_disk_is_shown`: no initial planning frame
  showing `(false, false)` within 30 seconds. It fails both in the materialized
  suite and the focused source-tree run with canonical TMPDIR. This is an
  unresolved failure, not a passing check or a reason to remove the assertion.

Original logs: `.lf/tmp/gate/run-55756/`; focused nextest run:
`d39dbe0e-f831-4ee7-9fe7-b1961c5febcf`. No production implementation was changed
for these findings. The gate remains non-green; delivery must retain that fact.
CI owns its full platform matrix. No release, installation, schedule change or
Task completion occurred. Release's child guidance still separates source checks,
publication and installed evidence; its goal and memory were read during realignment.

Checks: `uv run python scripts/test.py --reuse-passing` on `adf3f9e4b` with inherited LF/LOOPFLOW authority cleared: architecture, fmt, stable Clippy, website (76), Swift app/DTO/model/view tests (404) and boundaries PASS; materialized Rust 2333 PASS / 5 FAIL / 17 skipped; canonical-TMPDIR focused reruns resolve three failures, checkout-watch still FAIL; isolated copied-shell probe exits -9; disposable `lf monitor list --json`, `lf usage --json`, `lf doctor --json` PASS; full platform/installation matrix remains CI-owned.
