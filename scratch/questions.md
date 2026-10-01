# LOO-365 assumptions and open questions

Recorded 2026-10-01 during a headless implementation run. Each was the simpler
reversible choice; none has Jack Heart's confirmation.

1. **A recorded landing is what gets repaired, Task or not.** Jack's decision
   says "A PR with no Task is reported, not repaired." The watcher reports a
   failing PR that has no Task and no landing. A standalone PR someone delivered
   with `lf land` has a landing and no Task; it is still repaired, because that
   is today's behavior and the landing is an explicit request to deliver it.
   Reading the decision strictly would stop repairing those. One line in
   `ci_watch::respond` decides this.

2. **A failing Task PR nobody armed is reported, not repaired.** The repair
   contract (`ci-fix`, `admit_ci_fix`, the incident's reservation) is defined
   for a landing and ends by re-arming. Hosted CI also only selects landing
   candidates, so an unarmed PR rarely has a required check at all. Repairing
   unarmed Task PRs would need a repair that publishes without arming.

3. **The cron keeps Flow resumption and merge settlement.** Jack earlier decided
   to delete the cron with the watcher as the only clock; the supervising
   request narrowed that to the repair path. With no watcher running, failed
   landings are recorded and wait.

4. **Commands are flags, not subcommands.** `lf ci watch install` made the
   documented `lf install` shorthand ambiguous, so the service is
   `lf ci watch --install` / `--uninstall`, and state is `--status`.

5. **A second copy stands by rather than exiting.** A launchd service that
   exited would be relaunched every 30 s while Desktop's copy runs; standing by
   lets whichever copy survives take over. `--once` exits instead.

6. **Every landing gets the full landing check every five minutes**, even with
   no failed check, so a conflicting or stale head and a CI timeout are still
   repaired. This costs a few GraphQL reads per landing per five minutes.

7. **Desktop watches repositories open in a window**, not every repository in
   the portfolio list. It skips fixture and UI-test launches.

## Signal worth a metric

`lf ci` already reports detection latency; it was always empty because
`provider_completed_at` was null. With the watcher filling it, median and p95
detection seconds become a real measure of whether the watcher beats the
minute. No new instrument was needed.
