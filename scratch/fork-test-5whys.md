# 5 Whys — fork-test garbage leaks (rogue clients + stale worktrees)

## Symptom
- Repeated `/v0/waves/<fork-test...>/run` requests (429 spam).
- Accumulated `...fork-test-fork-*` worktrees left on disk.

## Why #1
Why did we get repeated run requests?
- A long-running fork-test client/process kept targeting a fork-test wave.

## Why #2
Why did that client collide with other runs?
- Fork tests reused predictable names (`fork-test`) and shared the same local daemon/port.

## Why #3
Why did stale test artifacts persist after failures/interrupts?
- Cleanup was mostly implicit (daemon-side), not guaranteed at test-script boundaries.

## Why #4
Why is daemon-side cleanup insufficient alone?
- Abrupt exits, concurrent local daemons, or DB/schema drift can bypass timely janitor cleanup.

## Why #5
Why do these failures become visible as local “garbage”?
- Tests run against developer-local state (shared repo parent + shared lfd port) without strict isolation + deterministic teardown.

## Root cause
- Fork tests were not hermetic enough: shared naming + shared local runtime + non-mandatory script teardown.

## Fixes shipped in this branch
1. Generate unique fork-test wave names per run (`fork-test-<8hex>`).
2. Add explicit best-effort teardown in script + pytest:
   - stop/delete test wave
   - remove matching local fork worktrees (`repo.wave`, `repo.wave-fork-*`).
3. Improve `dev.py lfd` preflight visibility:
   - print pid/repo/branch/wave for local listener on `:2486`.
   - kill stale listener before start (`-k` supported).

## Follow-up hardening (recommended)
- Add periodic janitor command for stale ephemeral worktrees and call it in dev workflows.
- Add lockfile around fork e2e to prevent concurrent local runs.
- Optionally route fork e2e through hermetic runtime (like API smoke) where feasible.
