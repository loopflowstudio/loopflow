# Active native Run discovery — slice review, 2026-09-24

The Session identity/recovery review encountered concurrent active-Run work in
this shared checkout. A production-like isolated CLI counterexample exposed a
continuity defect: a live native client with its existing per-Run receipt was
omitted from `runs --active` when the new `run-bindings` marker was absent. Its
Session remained `active`, but the active snapshot returned `runs: [], gaps: []`.
The exact owned child stayed alive. This models an existing launcher predating
the new binding writer; no live user record was changed.

Receipt and probe: [review2 evidence](session-run-evidence/review2/).

The bounded correction discovers native receipts through the existing Run
directory reader, independent of generic capture bindings. It reads manifests
only for nonempty client namespaces and never reduces historical event streams
or applies the history reader's date/count cap. Removed the redundant native
`ProviderClientGuard` binding writer. Generic/headless capture intervals retain
their separate exact Exec binding because native receipts cannot describe them.

The shared directory reader now propagates traversal failures instead of warning
and omitting subtrees; active observation returns that failure in `gaps`.
Discovery cost still grows with retained Run directories. This is not a latency
improvement or a bounded-cost claim: the required workspace performance baseline
must measure it before frequent polling/optimization. Do not regain apparent
speed by dropping clients from older launchers.

Added `native_client_receipts_do_not_require_a_new_capture_binding`: a real
owned `/bin/cat` client without a global marker must be found, then disappear
after exit even though its durable records remain. All five active-Run tests pass.
The first queued unit command could not acquire Cargo's build lock before source
changed; its filename `before.log` is not a pre-fix verdict. The standalone CLI
receipt above supplies the actual before counterexample.

Other active-Run changes, including missing-owner diagnostics appearing during
this review, belong to the ongoing implementation and were preserved. This
review does not approve unfinished Monitor/pane or benchmark integration.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Session names its Run | Required reference resolves before launch and through resume | Prepared Run identity persists; metadata open releases its lock before waiting | Three passing Session CLI tests; 16 configured public Run lookups | pass at CLI scope |
| Existing native clients remain visible | Old launchers need no new publication to be discoverable | Existing per-Run client receipts supply discovery independently | Isolated CLI before/after; live configured comparisons | pass after correction |
| Exact active Task Runs | Waiting clients remain active; dead clients and different Tasks do not join by checkout | One verified process observation joins exact capture or native ownership and typed Work | Five active-Run tests, including shared checkout and sequential Exec intervals | pass locally; configured inventory incomplete |
| Truthful incomplete evidence | Missing ownership or failed reads cannot claim healthy emptiness | Explicit gaps survive successful snapshots | Configured result reports one live Exec without a Run binding; shared DTO fixture | pass at recorded boundaries |
| One data authority | Reuse Run, Exec and native client records | Removed redundant native binding writer; generic capture interval remains necessary | Source trace and negative searches | pass for this correction |
| Complete canvas | One outline, Monitor beside retained terminals, measured navigation and workspace experience | Outline has prior bounded review; Monitor consumer and both measurement runners remain absent | Current design and reachable Mac source | gap |

## Configured and production-like proof

The isolated before trial returned an active Session while `runs --active`
returned an empty list with no gaps after removing only its new discovery marker.
The owned stand-in provider stayed alive. The corrected trial uses the real CLI,
temporary Home/store and a local stand-in OpenCode executable waiting on stdin.
It publishes no global marker at all: both reads find the same Run and waiting
PID, with no gaps. The owned child exits successfully through its stdin afterward.
Neither trial touches user clients or proves a vendor conversation or native UI.
Receipts: [before](session-run-evidence/review2/preexisting-client-before.json),
[after](session-run-evidence/review2/preexisting-client-after.json),
[executed probe](session-run-evidence/review2/preexisting_client.py).

Read-only queries explicitly selected the installed development Home used by
iteration 18, with aligned control/data paths. Six active Runs were observed.
All five Sessions reported active in the preceding Session read occur in that
snapshot. All 16 returned Sessions, including one human Flow boundary, resolve
through `lf runs <run_id> --json` to their exact required Run IDs. These sequential
observations are not an atomic inventory or UI demonstration. One live Exec has
zero current Run identities and is reported in `gaps`; the configured inventory
is therefore incomplete. No identity is guessed from checkout, title or age.
[Configured receipt](session-run-evidence/review2/configured-read.json).

## Source review and verification

Read the Task directive, latest canvas amendments and Done When, prior review,
and the required Session identity/recovery paths. `lf task diff` returned a
truncated patch. The unrestricted tracked patch at
`/tmp/loo291-session-review2-tracked.patch` contains 462 sections: 451 identical
to review 18's final snapshot and 11 changed. Inspected those changed Rust/Swift
sections and the new active reader, Swift DTO and shared fixture separately.
This review's discovery correction and DTO import fix follow that snapshot.
Concurrent outline/active-Run work is preserved; prior UI receipts remain tied
to their own source, and this review makes no fresh UI claim.

Static analysis found the new external DTO fixture importing private
`run_record`. Exported its public wire types through the existing `commands::runs`
API, beside the other Run DTOs, and updated the fixture import. The internal
Run-record module remains private.

Commands and receipts:

- `cargo test -p loopflow --lib run_record::active::tests`: five pass,
  `/tmp/loo291-native-discovery-after.log`.
- `cargo test -p loopflow --test session_cli_tests`: three pass,
  `/tmp/loo291-session-review2-cli.log`. Covers prepared identity, first launch,
  native resume and metadata access while a provider waits.
- Corrected standalone probe exits zero,
  `/tmp/loo291-preexisting-client-after.log`.
- `cargo test -p loopflow --test dto_fixtures
  active_runs_preserve_identity_waiting_clients_and_incomplete_evidence`:
  one pass, `/tmp/loo291-session-review2-dto.log`.
- `cargo clippy --all-targets -- -D warnings` passes after the public import
  correction, `/tmp/loo291-session-review2-clippy-final.log`.
- `cargo fmt --check` and `git diff --check` pass. No broader test gate or
  unchanged Swift suite was rerun.

Negative searches retain one Podium Session/roadmap reader and one root workspace
registry. Former temporary identity-binding writer, SessionScope and
FlowResolutionAction remain absent. There is no production `query.activeRuns`
consumer or Monitor pane yet. Native discovery reads receipt namespaces and
manifests, not historical output/usage; its directory traversal remains a real
performance cost, not an optimization claim.

## Disposition

The corrected shared contract advances the agreed canvas. Continue with a
Task-bound Monitor pane in the existing multiplexer and Podium's existing read
ownership; preserve explicit incomplete evidence. Implement the two defined
rendered/usable performance runners for hierarchy navigation and Task workspace
opening/switching. Measure the retained-registry traversal before choosing its
first optimization. Neither source review nor these CLI proofs replace the
human's simplified-UI demonstration or retained external-work obligations.

No publication, landing, Task completion, installation, PM mutation or LOO-293
closure occurred. The [review-slice skill](/Users/jack/.agents/skills/review-slice/SKILL.md)
permits publication “When all applicable `Done when` claims hold and the slice is
coherent.” The complete canvas's Monitor and measured-experience claims remain
unmet; this review leaves publication pending.
