# Finish the Run cutover — LOO-370

## Accepted direction — October 5

Jack Heart authorized autonomous source development and landing. Infrastructure,
under that delegated direction, selected one retained opaque `runs/` capture root;
the updated Task brief and steers explicitly accept this exception. This changes
the earlier physical-layout requirement; it does not satisfy it by conversion.
No relocation, parallel root, alias, offline boot/storage owner or installation
change belongs in this Task. Installed-Home writes, interruption and release remain
outside this source-delivery authorization.

Prior design, alias counterexamples, wrong-inode restoration race and unproved
Mac boot custody remain in `6fcdbe9da0b47ef95f1f92ebdb259401a327cd46:scratch/finish-removing-the-retired-run.md`.
The failed permission approach is discarded, not vindicated by the scope change.

## Owners and preservation

AgentSession owns conversation identity and native history; FlowSession owns its
captured progression; Exec owns process ancestry. Capture artifact keys select
subordinate Session history, never mutation authority. Keep SQLite records,
immutable receipt fields, artifact keys, feedback, usage and native references.
`record_dir` continues deriving `runs/<shard>/<artifact-key>` for every capture.
Missing payload cannot erase resumable SQLite identity. No schema migration.

Runtime changes already use `LF_CAPTURE_KEY` for history and resolved AgentCaller
for Session/Exec provenance. Native resume exports its current driver provenance.
Session mutations take durable IDs and fence stale providers/captures in SQLite.
Checkpoint composition and agent progress use resolved execution, never old
variable presence. Public history/replay retain artifact selectors. Git-operation
fields name Trace/Exec while their released serialized receipt names remain.

## Delete — do not maintain

Removed `.github/workflows/capture-exclusion.yml`, `scripts/test_capture_exclusion.py`
and `tests/e2e/capture_exclusion.{py,Dockerfile}` with their TESTING.md instructions.
Their negative evidence survives at 6fcdbe9da0b47ef95f1f92ebdb259401a327cd46; they are not runtime acceptance.
Delete obsolete commands/identity names in current help, tools and docs. Keep
historical cohort decoding only where an actual reader consumes it. Preserve
legacy environment names solely in scrubbing and rejection tests.

Already removed: unused engine events/errors and worktree helpers, duplicate replay
readers, mutation capture aliases and unused provider-fill wrappers. Earlier focused
proofs remain at `71a784cdf5135fad8b26a21ddd6ea1ab354c4a93` and runtime compression
at `d537a095cfe3fbb8e6e48cc04ae1b1cb1f51cd58`; they are not a complete final gate.

## Implementation and remaining gate

The semantic audit now covers current Rust/Swift/Python/CLI/DTO/scripts/skills/docs
and configuration. Wave detail exposes required `history` in Rust, Swift and JSON
fixtures. Telemetry labels/metric IDs name Sessions; ablation/check-cost interfaces
name captures. CLI help names durable/native conversation selectors;
monitor `--input` and replay keep capture selectors. The abandoned probes and their
exclusive workflow/runner/docs are removed. No schema or physical layout changed.

`tests/e2e/capture_history.py` verifies a checksum-pinned published v0.13.3 archive,
then creates populated capture history and a waiting review through that CLI in a
temporary Home. Candidate reads preserve exact paths/bytes, event rows and usage;
resume retains the native conversation, replay creates independent history, released
feedback survives review completion and Flow settlement, and nested commands retain
the actual parent Exec. Provider/terminal transport is simulated. This is executed
runtime preservation, not installation or migration evidence. Meaningful SIGINT and
stale-capture/provider replacement remain covered by focused Rust public tests.

The retained-reference inventory and measured source delta are in
`docs/architecture-reference.md#retained-encoding-inventory-loo-370`.
Active comparison base is `8ea0bec9cf4b0c08ca17c52e57de059000a7b0e3`; original
pre-integration base was `a278d6bc1bd4373f78f27027b8ae249100ef14d3`.

Gate remains: run affected Rust/Python suites, architecture and immutable-migration
checks, plus required headless Desktop verification once on the final candidate.
Reuse applicable focused evidence. The filtered Desktop build/tests passed, but its
optional display-session TaskHistoryProofTests suite was skipped; no rendered
Desktop or configured provider acceptance follows. The three installation tests
listed in questions.md are forbidden on this host; integrate PR #1444's isolated
runner or retain isolated CI ownership. Full verification and delivery belong to
the saved pursuit's following steps.

Review findings fixed: stale mutation-selector help; Wave DTO `runs` field still
misnamed its Session history; scorecard and capture-analysis flags/labels still
exposed the retired object. One physical root and existing SQLite fences remain.
Compression removed unused history-command DTO exports and corrected the remaining
retired monitor example. Required DTO exports stay at the public command boundary:
Session storage is private. The preservation fixture drops a redundant history
read/connection and verifies the review's recorded completion time. Its provider
launch log precedes resumability; completion now waits through the public operation's
Session lock before stopping that client. The failed early-exit run remains evidence
of fixture ordering, not a reason to change production timing.

## Adjacent evidence

Release child GOAL.md and MEMORY.md were read October 5. LOO-292's installed
acceptance does not prove this cleanup; LOO-285 still lacks two qualifying automatic
settlements. Upstream #1441 removes the retired UI receipt prerequisite and repairs
CLI smoke. Preserve its actual headless/public checks and release schedule.
Configured installation/provider/Desktop acceptance remains separate from fixtures.

Prior implementation checks remain at `c2f464b5a4333b50dd67823585d236b63115223d:scratch/finish-removing-the-retired-run.md`; their focused runtime and headless Desktop evidence remains applicable, with the same acceptance limits.

Check (October 5, compression): `cargo test -p loopflow --test dto_fixtures --test active_runs_watch` passed (19); `uv run python tests/e2e/capture_history.py --released-archive /tmp/loo370-release-fixture/lf-aarch64-apple-darwin.tar.gz --candidate target/debug/lf` passed after the fixture-ordering repair; `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, Ruff and `git diff --check` passed; full affected gate and isolated installation checks remain gate/CI-owned.
