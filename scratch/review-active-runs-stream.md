# Shared automatic Run observation review — 2026-09-24

The stream advances the accepted design. One retained Rust reader now supplies
automatic observations through Swift while the existing multiplexer preserves
native input. Review reproduced and fixed a diagnostic-loss race. Configured
vendor activity, human composition acceptance and the complete canvas obligations
remain open; this is not approval to publish the full change.

## Finding fixed

Stdout closure immediately finished the stream with “Active Run reader closed its
output,” discarding a useful CLI stderr message. The process-exit handler could
also run before the stderr callback. This hides the actual reason for startup
failure, leaving Retry without an actionable diagnosis.

An owned shell emits a specific fixture diagnosis and exits, or closes stdout
before waiting for shutdown. The first immediate-exit observation passed; the
expanded two-case regression then failed both cases with the generic closure
message. This establishes an ordering race, not a claim that every exit loses
its diagnosis. Both original logs are retained.

Both end paths now drain available stderr on the existing serial queue before
choosing the error. The existing 16 KiB diagnostic bound and nonblocking drain
remain. Already-canceled stderr sources are not read again. No new timer, reader,
retry, process owner or Session behavior was added. The two-case regression passes
with the actual diagnosis preserved and the owned reader exited.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Automatic observations | Discover existing clients, follow publication/exit and rescan | One foreground CLI streamed through production Swift transport | Fresh isolated-Home CLI test, owned cat clients dated 2020 | pass locally |
| Shared lifetime | Navigation keeps one reader; teardown reaps it | Podium first-demand retention, awaited cancellation and generations | Two-Task navigation, held Home replacement, deallocation tests | pass locally |
| Recovery and missingness | Retain useful rows and time; incomplete cannot mean empty | Scanning retains last-good; fatal discovery/transport pauses until Retry | Focused Monitor tests and native scanning/empty transitions | pass with fixture frames |
| Pipe failures | Bounded delivery, explicit error, cleanup without stopping providers | Off-main serial decoding, frame/diagnostic bounds, latest-only delivery, owned-process cleanup | Fragmentation, backpressure, oversize, Home mismatch, silence and TERM-ignoring child tests | pass locally |
| Actionable startup failure | Preserve the CLI diagnosis despite callback ordering | Both end paths drain pending diagnostics | Two failures before correction; both cases pass after | pass after correction |
| Native continuity | Same Session draft, focus and responding companion through updates | Existing retained surfaces and Monitor responder | Mounted production SessionsView/Ghostty, chapter transfer, zoom/resize and PTY replies | pass with fixture transport and real PTYs |
| Bounded discovery | Warm work does not enumerate historical directories | Previously reviewed retained Rust reader unchanged | Matching Rust source hashes and prior cost/ownership receipts | prior local proof; no new benchmark |
| Configured acceptance | Positive vendor Run changes/recovery beside retained panes; usable composition | Implementation exists; this review exercises isolated clients and mounted fixtures | No new configured vendor or human trial | gap |
| Complete canvas | Rendering/hitch/phase evidence, budgets, chapter ownership correction and external trials/edit | Existing narrower receipts remain | Governing directive and design ledger | gap |

## Verification

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 --no-parallel \
  --filter 'ActiveRunsObservationTests|ActiveRunsLifetimeTests|TaskMonitorTests|TaskMonitorProofTests|DTOFixtureTests/activeRunsFixture'
```

**17 tests in five suites pass.** The real CLI trial observes a second Run without
Refresh, removes it after client exit without deleting its receipt, rescans, and
awaits reader exit while the first client remains alive. It uses this checkout's
CLI and a temporary Home, not a configured vendor conversation. The mounted native
proof uses fixture frames and real Ghostty PTYs; it is not compositor or human
acceptance evidence. No executable edits followed the final focused run.

The earlier implementation's 19 source hashes, seven artifacts and two binaries
all matched before review edits. Discovery's nine artifact hashes and its Rust
sources also match; the four superseded Swift hashes are covered by the stream
receipt. Prior cost results retain their original executable attribution.
No Rust or performance rerun was warranted by this Swift diagnostic correction.

The supervised compile-only fallback plan also passes resource preflight,
Xcode app/test `build-for-testing` and postflight. No hosted UI suite ran.
The correction patch passes its added-line whitespace check. Compilation and
final source/artifact hashes are recorded in
[the review receipt](active-runs-stream-review-evidence/receipt.json).

## Source and ownership

Read the cached governing LOO-291 directive through `lf pm show --wave product
--no-sync --json`, the full workspace and discovery designs, implementation,
compression report, previous reviews and Desktop direction. The cache is not a
fresh PM sync. The current slice is automatic shared transport; the full target
also requires configured proof and the broader canvas obligations above.

Recovered the complete Task diff inventory through `lf task changes` and 240
bounded `lf task diff` queries. All 836 sections are accounted for: 791 match
the prior discovery review and 45 changed/new. The historical 1.1 MB roadmap
fixture exceeds the CLI cap; its complete file parses and matches the prior
full-content hash. Reused preceding reviews for unchanged sections and inspected
the stream delta, tests, documentation and integration seams. The previous
manifest-diagnostic correction remains its own contribution. The capture overlaps
this review's regression addition; the final correction has a separate patch and
source hashes. This is an incremental review, not a fresh full-branch test gate.

Traced native receipt/capture evidence through the retained reader and streaming
CLI, RegistryQueryLocal launch configuration, nonblocking transport, Podium
publication, Task filtering, root teardown/wake and retained panes. Checked the
required DTO mirrors and fixtures. Launch configuration preserves Home/account
selection while clearing inherited execution/terminal context; frames validate
canonical Home and reject Task-scoped responses.

Negative searches retain one production Podium `watchActiveRuns` caller, one root
workspace registry and one production capture-binding writer. The Mac one-shot
`activeRuns` method/caller is absent. Rust one-shot and streaming reads share the
reader/projector. No environment locator, retained-history fallback in the warm
path, durable Watch/index, provider writer, per-pane reader or competing layout
was introduced. Shared liveness remains distinct from Session identity and local
terminal attachment. Cancellation targets only the transport's owned Process.

Existing dirty contributions were preserved. Pre-edit copies of the two edited
Swift files are at `/tmp/loo291-stream-review-before`; no blanket checkpoint or
staging claimed another writer's work. The durable Desktop direction now records
the stderr/exit ordering lesson.

## Next boundary and disposition

Demonstrate positive configured vendor Run appearance/disappearance and recovery
beside retained Session/companion panes using fresh owned identities. Keep the
complete chapter objective/target correction, compositor/hitch and correlated
phase measurements, configured costs and budgets, human composition verdict, and
the original external-work/edit obligations open. Do not substitute another cat
fixture or repeat the existing capture/OCR baseline for those outcomes.

The invoked [review-slice skill](/Users/jack/.agents/skills/review-slice/SKILL.md)
permits publication “When all applicable `Done when` claims hold and the slice is
coherent.” Configured discovery acceptance and the full canvas claims remain
unmet. No publication, installation, PM mutation, provider transfer or Task
completion occurred; LOO-293 and sibling worktrees were not changed.
