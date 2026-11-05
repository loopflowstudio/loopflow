# LOO-722 inspection repair and remaining acceptance

## Outcome and accepted policy

Jack Heart's outcome is reliable hourly coding cleanup, recurrence prevention and
successive measurements across repositories and tooling. Publication alone does
not finish this Task. Preserve its identity and prior delivery history: PR #1820
and PR #1821 (merged as `3e3a653b2b19243b69c11c6843e09779aa846360`).

Keep the accepted 4h/1h/15m idle windows, 24h minimum owner termination, 24h
successful evidence and seven-day failed/unknown evidence retention. Source,
unpublished work, durable history/databases, active dependencies, saved evidence,
project reservations and Chrome owners remain protected. Non-coding cleanup needs
explicit approval. Preserve complete owner evidence and PID/start verification;
unknown inspection fails closed, without retries or partial-inventory deletion.

## Delete — do not maintain

The diff replaces `_schedule`'s Background classification with Standard and removes
`_processes`/`_activity`'s special 10/30-second bounds. Both now use `_command`'s
two-minute limit, capped by the remaining run budget. No deletion targets remain
in this repair. Keep the same hourly job, clean environment, versioned installation,
locking, overall budget and owner teardown. No second runner or broader authority.

## Evidence behind the repair

The ordinary **2026-11-04 17:17:09–17:19:28 PST** cycle on merged main exited 1:
initial `ps` exceeded ten seconds, before discovery or native maintenance. No
candidates, signals or removed allocation; protection checks passed. Free space
fell independently by 2,995,564,544 bytes to 193,018,949,632 bytes, with zero target
shortfall. Accounting measured 29 of 134 roots, left 105 unknown and compared 27
last-known values, five growing. The earlier successful manual run and 101-test
gate for discovery/Docker repair did not establish ordinary-calendar acceptance.

Read-only reproduction under build contention: `taskpolicy -b` made both exact
ps/lsof commands exceed 30 seconds; foreground probes finished in 0.68/1.22 seconds.
A clean-environment launchd probe reproduced the ten-second ps timeout with
Background; Standard completed ps/lsof in 2.49/15.69 seconds. A second Standard
probe took 8.68 seconds for ps but exceeded 30 seconds for lsof. Real teardown also
hit ten seconds on a two-PID ps query. Observed load exceeded 200. This supports
removing both background throttling and the short bounds; it does not prove every
timeout has that cause. Standard's light limits are documented by local
`launchd.plist(5)`.

Installed repaired files match source. The unchanged minute-17 job's manual
kickstart ran **17:55:40–17:58:52 PST**, exit zero: 291 candidate decisions, no
failures, signals or removed allocation. Six protected file hashes, three root
identities, private configuration, schedule and all five installed digests passed
checks. Free space fell independently by 2,951,847,936 bytes to 175,365,357,568 bytes;
shortfall remained zero. Accounting measured 32 roots, left 102 unknown and compared
32 last-known values: ten grew, none shrank. Coverage, intervals and overlapping
roots prevent a containment conclusion.

`uv run pytest python/tests/test_disk_cleanup.py -q -k 'launchd_install_replaces or stops_real_demo_owner or failed_open_file_inspection or wall_budget_reports'`: five passed; Ruff check/format passed. These results remain applicable: compression changed only prose. `git diff --check` passed; `lf context` is within both limits. Gate owns affected suites.

The real owner-tree test verifies graceful/forced descendant termination while
preserving separate history. LOO-725 comment
`1a4fd20f-9bf3-43fa-aaea-9a76629d0be7` concerns a provider reparented *before*
inspection, with no remaining runtime handle or discoverable ancestry. Current
inspection cannot authorize its termination; shared history or repository ancestry
provides no substitute. Descendants discovered *during* teardown retain their
identities. Transient hook PIDs missing from ps still fail closed. LOO-725 owns
runtime changes; this repair does not redesign ownership.

## Remaining acceptance

Obtain the ordinary **18:17 PST** receipt on final bytes, repeat protection checks,
and assess successive coverage and growth. The manual run does not replace this.
Keep unknown measurements visible and address recurring growth with repository-owned
prevention where evidence calls for it. Sustained containment remains unverified.
Gate and the authored delivery/verification steps own checks, publication and
ordinary-cycle acceptance. The probe service was unloaded and its children stopped;
compression started no processes requiring shutdown and changed no installed bytes.

## Evidence locations

Private evidence root:
`~/.lf-machine/maintenance/disk-cleanup/evidence/20261104-LOO-722/`.
`discovery-repair/verify-pass-2/` holds the failed ordinary receipt and protection
checks. `inspection-repair/` holds probes and prior notes; `manual/` holds the repaired
receipt, usage history, launchd status, schedule and preservation summary.
`inspection-repair/verification-before-compress.md` preserves the full uncommitted
note before consolidation. Personal inventories remain private.

After the ordinary job finishes, from this checkout run
`uv run python ~/.lf-machine/maintenance/disk-cleanup/evidence/20261104-LOO-722/inspection-repair/capture.py ordinary 1793845020`.
The minimum epoch is November 4 at 18:17 PST; this command collects evidence and
asserts freshness, success and preservation without initiating cleanup.

Earlier accepted design/demo and gate detail remain at
`7e969d4f6e:scratch/loo-722.md`,
`7e969d4f6e:scratch/disk-cleanup-demo.md`, and
`8086c21a2d:scratch/disk-cleanup-verification.md`.
