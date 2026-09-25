# Shared automatic Run observation — 2026-09-24

## Result and scope

Monitor now updates automatically from one foreground Rust reader per used Podium
window. Selecting another Task/repository, hiding Monitor or closing its pane does
not cold-start another reader. Session/companion panes keep their existing owners.
This completes the Swift transport slice of the discovery design. Configured
vendor-provider and human acceptance remain separate next work.

The finish line was automatic exact-Task observations with explicit recovery,
bounded pipe delivery and owned-reader cleanup while native input survives.
Refreshing a one-shot process, accepting a model assignment as native input
proof, or manufacturing a confirmed empty during recovery would not count.

## Ownership and review

- Removed `RegistryQuery.activeRuns`; the Mac consumer uses `watchActiveRuns`
  exclusively. The public Rust one-shot CLI is unchanged. RegistryQueryLocal
  launches the existing foreground command through shared GUI process preparation.
- Local transport serializes nonblocking stdout/stderr draining and decoding on
  its own queue. A frame is limited to 16 MiB; diagnostics retain 16 KiB; decoded
  delivery holds one latest snapshot. Requests coalesce, with rescan taking priority.
  Ten seconds without a complete frame ends the observation with an explicit error.
- Podium starts on first demand. Its weak consumer and generation checks prevent
  model retention and late publication. Window cancellation awaits reader exit.
  Closing stdin is graceful; after one second it sends TERM, after two KILL to
  the exact owned Process, never a process group or provider.
- The captured launch environment includes Home/database selection. Explicit Home
  overrides and subsequent frames are checked against canonical Home identity;
  absent an override, the first Rust frame establishes the resolved Home. Swift
  does not reimplement installation selection. Helper and installation-file
  replacement invalidate that launch configuration. Podium clears old evidence,
  awaits cancellation, then starts its replacement; ordinary failures require Retry.
- Wake requests rescan. Scanning retains the prior useful observation and its
  original timestamp. Ready frames with gaps remain incomplete. Fatal discovery
  frames retain their diagnostic and stop the stream, avoiding a later silence
  timeout overwriting the actual cause. Retry starts one new reader.

Review kept Run identity, transport lifetime, reading freshness and terminal
ownership distinct. No durable cursor/index, provider writer, additional pane
tree or per-pane poller was added. Existing dirty Rust/discovery/flow contributions
were preserved. Before-edit copies of the touched existing Swift files are at
`/tmp/loo291-stream-before`; no blanket checkpoint staged another writer's work.

## Focused evidence

`swift test --package-path swift -Xswiftc -gnone --jobs 4 --no-parallel --filter
'ActiveRunsObservationTests|ActiveRunsLifetimeTests|TaskMonitorTests|TaskMonitorProofTests|DTOFixtureTests/activeRunsFixture'`
passes **16 tests in five suites**, including the five invalid-frame cases.

- A real local CLI in a temporary Home discovers an existing waiting cat client
  dated 2020, publishes a second Run without a manual refresh, removes it after
  client exit without receipt cleanup, and recovers after rescan. Cancel returns
  after reader exit while the original client remains alive.
- Fragmented JSON and stderr beyond pipe capacity, latest-only burst delivery,
  invalid JSON, oversized/no-newline frames, initial and later Home mismatch,
  partial-output exit, ten-second silence, and a TERM-ignoring child all have
  explicit outcomes. The forced-cleanup check leaves another owned client alive.
- Two Task Monitors share a reading through navigation, retain positive rows
  during scanning, distinguish incomplete from empty, and require Retry after
  failure. Held cancellation proves old-Home evidence clears before replacement
  starts. Model deallocation cancels even when its Monitor is hidden.
- Mounted production SessionsView/Ghostty panes retain original surfaces, actual
  focus, unfinished draft and responding companion through automatic positive,
  scanning and empty frames, chapter transfer, resize/zoom and Session return.
  Those snapshots use fixture transport; the PTYs are real. This is not a
  configured vendor conversation or a visual composition verdict.

The initial real-CLI assertion compared `/var/...` with Rust's canonical
`/private/var/.../`. Corrected the fixture to compare canonical paths; transport
already did so. The original failure is retained. Review also changed incomplete
frame search to inspect only new bytes, avoiding repeated scans of an oversized
unterminated line. Strengthened invalid-frame assertions require the actual
resource/Home/parse diagnosis rather than accepting any eventual timeout.

SwiftPM builds the native path. Xcode `build-for-testing` passes for app and test
targets without Ghostty, through the existing supervised/resource-checked runner
with only the loopflow compile plan enabled. The final incremental compile follows
the last production edit. No hosted UI suite ran.

An initial `scripts/test.py --loopflow` unexpectedly selected broader affected
suites. It reported architecture drift (`exec:/bin/cat`) and a Python failure;
the owned runner was interrupted. That partial run is not a gate pass or a
diagnosis of this Swift slice. The subsequent compile-only plan passes. Logs,
source/binary hashes and the bounded contribution patch are in
[the evidence receipt](active-runs-stream-evidence/receipt.json).

## Remaining boundary

The Rust cold/warm matrix remains its separately attributed earlier measurement;
no new discovery speedup or rendering budget is claimed. The desktop benchmark
fixture now supplies the streaming contract, so its prior timing receipts remain
tied to their old measurement source. No benchmark rerun is claimed.

Configured positive vendor activity/recovery beside retained panes, compositor
and hitch evidence, correlated phases, configured costs/budgets, the complete
chapter objective/target correction, human composition acceptance, external
trials and authorized directive edit remain open. No installed app, PM state,
provider transfer, publication, Task disposition or LOO-293 worktree was changed.
