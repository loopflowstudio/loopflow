# LOO-708 process footprint demo — 2026-11-02

Decision pass 1: **advance**. The agent made this decision under the caller's
explicit instruction to stand in for Jack Heart during this headless demo.
Jack Heart supplied the requirements; no new personal observation or approval
from Jack Heart is claimed. No design change is proposed.

The [accepted design](loo-708-processes-stay-small.md) requires fewer retained
processes while preserving resume, takeover, exit recording and Graph behavior.
The [pre-shipment review](loo-708-review.md) records the delayed-handoff repair
and passing transport checks. This pass adds current release-build measurement
and a terminal walkthrough at `238b70ef9cea6b9b16605afc18078a7e92178175`.
It does not rerun the earlier suites or replace their evidence.

## Observed experience

Built with `cargo build --release -p loopflow --bin lf`. In a disposable Machine
and local Git repository, ran `lf -a claude resume --prepare --json`, then the
returned `open_argv` unchanged in a PTY. This exercises the command Desktop
receives, including ordinary repository conversation preparation. The provider
was an isolated shell stand-in that printed readiness and waited for terminal
input. PATH contained only that stand-in and system tools; no authenticated
provider or new agent turn ran.

The [final transcript and measurements](loo-708-demo-evidence.txt) show:

- Preparation launched no provider. Opening left owner PID 4223 and relay PID
  4148. Provider PID 4251 was the owner's direct child, with no additional lf
  child or retained launcher beside it.
- `footprint --noCategories -p PID` measured owner 21 MB and viewer 2512 KB
  (viewer auxiliary `phys_footprint` 2544 KB). Two earlier attempts on the same
  build measured owner 20 MB and viewer 2560 KB. These are footprint, not RSS.
  The design's historical pre-change comparison is 16 MB owner + 17 MB inner
  connect + 58 MB launcher; those baseline numbers were not remeasured here.
- `lsof -p 4148` showed no `loopflow.db` descriptor in the live relay.
- Ctrl+] exited the first viewer successfully. The owner and provider survived;
  preparing the same Session reported `detached`.
- Reopening restored `DEMO READY`. A third terminal took control; the preceding
  terminal printed `This conversation moved to ttys072: lf resume …` and exited.
  All attachments used the same provider PID; its launch log had exactly one entry.
- Sending `bye` printed `DEMO EXIT bye`, the continuation command, and exactly
  one complete JSON Session record with the original ID and state `closed`.
  All three viewers exited 0. The ledger completed the provider and launcher;
  it recorded one completed attachment exit and one capture settlement.
- Cleanup verified that neither demo owner nor provider remained running.

## Failed probe attempts retained

The [first attempt](loo-708-demo-first-attempt.txt) timed out waiting 15 seconds
for takeover while leaving terminal output unread. The
[second attempt](loo-708-demo-second-attempt.txt) drained the takeover terminals
and observed the correct moved notice, but timed out waiting 30 seconds for the
final JSON-producing relay with its terminal output unread. Cleanup terminated
the remaining demo processes in both attempts; those were not successful exits.

The final probe drained terminal output while awaiting exit, as a terminal
consumer does, and passed without changing production code or increasing the
deadline. This supports attributing the two timeouts to the probe's output
backpressure, rather than an implementation repair. The failed evidence remains
linked above; it is superseded only for the demo outcome.

## Judgment and remaining proof

The preceding implementation intended to eliminate the retained launcher and
viewer heap without losing their remaining work. Current process ancestry,
footprint, takeover output, JSON continuation and settlement records demonstrate
that progress. No concrete repairable implementation gap emerged. Under Jack
Heart's accepted preservation direction and the delegated decision authority,
advance to the remaining delivery checks instead of returning to implementation.

This is configured release-CLI/PTY-path evidence with a provider stand-in,
not rendered Desktop or authenticated-Claude acceptance. No screenshot or
interactive HTML page was produced on this headless surface. Installed
Desktop/Claude acceptance remains unproved. Reboot recovery and Graph sequencing
retain the earlier transport-suite evidence; this walkthrough did not repeat
them. Gate still owns affected lifecycle, launch-contract, startup-cost and
journal checks identified in the design. Integration with LOO-705/710/712 must
preserve successor selection and preview cleanup; wire encoding remains LOO-712's
scope. There is no unresolved product question requiring a person here.

Checks: release build passed; isolated prepared-command terminal demo passed;
JSON record parsed and matched; all demo processes stopped. Durable evidence is
`scratch/loo-708-demo-evidence.txt`; review record is `scratch/loo-708-demo.md`.
