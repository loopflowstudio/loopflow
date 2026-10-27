# LOO-629: cron runs a truncated command (implemented on branch, 2026-10-27)

## Problem

Jack's 10:00 release-run cron fired on 2026-10-27 but never ran lf. macOS cron
(Vixie, `MAX_COMMAND 1000`) reads at most 999 bytes of a crontab command. The
bootstrap from #1732 inlines `cd … && env -i HOME=… PATH=<host PATH> … <gate> …
>> log` into the line, and the host PATH pushed the line to 1,408+ bytes. Cron
ran a prefix that ended inside PATH. `env -i` printed its assignments to
/var/mail/jack, and `.lf/logs/release-run.log` was never created.
`crontab -l` stores and shows the full line, so the bootstrap's
"installed" check passed while the line cron actually ran was broken.

The current host crontab has both loopflow lines over the limit (telemetry
1,430 bytes, release 1,419). The 2026-10-28 09:00 telemetry run will fail the
same way unless it is reinstalled. Both launchd jobs are already retired. The
only other crontab content is unrelated entries, if any.

## Demo

```bash
cd ~/src/loopflow && scripts/bootstrap-cron-host.sh infrastructure
crontab -l | grep loopflow:infrastructure
#  0 9 * * * /bin/sh /Users/jack/src/loopflow/.lf/cron/infrastructure.telemetry-daily.sh # loopflow:infrastructure:telemetry-daily
#  0 10 * * * /bin/sh /Users/jack/src/loopflow/.lf/cron/infrastructure.release-run.sh # loopflow:infrastructure:release-run
cat .lf/cron/infrastructure.release-run.sh   # the full unattended command, readable
```

Each line is about 120 bytes. Bootstrap's own telemetry proof runs the
telemetry wrapper, the file cron runs, and prints the tail of
`.lf/logs/telemetry-daily.log`.

## Approach (chosen)

Bootstrap generates one wrapper per job and installs a short crontab line that
runs it.

- Wrapper path: `$repo_root/.lf/cron/<wave>.<job>.sh`, mode 0755. Its content
  is `#!/bin/sh`, then `cd <repo> && exec env -i HOME=… USER=… PATH=<host_path>
  LF_HOME=… TMPDIR=… LANG=… <gate> --wave <wave> --batch graph <job> >> <log>
  2>&1`. Values are `%q`-quoted from the existing `minimal_env`, as they are
  today. The wrapper is a host snapshot from bootstrap time and contains no
  secrets, just as the crontab line has none today.
- Crontab line: `<schedule> /bin/sh <%q wrapper> # loopflow:<wave>:<job>`.
- Before `crontab -` runs, `job()` refuses any line of 1,000 bytes or more and
  names the cron limit. If it refuses, the crontab is not written and launchd
  stays untouched. This turns a silent 10:00 failure into a loud bootstrap
  failure, and it can only trigger for a repo path near 900 bytes.
- `.gitignore` gains `.lf/cron/`.
- The "configured-path telemetry run" step runs `/bin/sh <telemetry wrapper>`
  instead of re-spelling the command. On failure it prints the tail of
  `.lf/logs/telemetry-daily.log`. The proof and the schedule then share one
  artifact.
- `release/CRON_HOST.md`: the crontab example shows the short lines. A sentence
  explains that the command lives in `.lf/cron/*.sh` because cron reads only
  999 bytes of a line.

Unchanged: the gate path pin, implicit and explicit Machine placement, the
preflight order (tools → accounts → publisher check → crontab → launchd
retire), the publisher, LF_HOME, and the env contents.

## Alternatives rejected

- **Crontab `PATH=` env lines.** Every crontab entry, unrelated ones included,
  would inherit them, and Vixie also limits env strings (MAX_ENVSTR). This breaks
  "preserve unrelated entries".
- **Committed `scripts/cron-job.sh <wave> <job>` that rebuilds env at run time.**
  Cron's own environment has a bare PATH, so the host PATH would still have to
  be persisted somewhere. It also lets a mid-change checkout alter the command
  between bootstrap and 10:00. A generated snapshot is simpler and can be
  inspected.
- **Trimming PATH.** That is fragile. A dev setup that grows PATH by 200 bytes
  brings the failure back.
- **LF_HOME for wrappers.** The jobs are bound to this checkout and its logs
  already live in `.lf/logs/`, so keeping cron artifacts beside them is easier
  to find.

## Risks found

- The truncation mechanism is confirmed: the mail body is an exact 999-byte
  prefix, and both current lines are over 1,000 bytes.
- `/bin/sh` on macOS is bash in POSIX mode. The `%q` output of bash 3.2/5 uses
  backslash escapes for spaces, which POSIX sh accepts. A test covers a path
  containing a space.
- Wrapper deletion is not a risk: `.lf/cron/` is new, and nothing in lf cleans
  `.lf/` subdirectories that it doesn't own (`.lf/logs/` survives the same way).
- Release proof timing: v0.14.19 was published manually (2f329d7394). The
  Infrastructure primary conversation owns correction PR #1737. The next
  scheduled 10:00 run publishes if main has unreleased commits, or reports no
  changes. Neither outcome can be produced here without a competing publisher.

## Tests (python/tests/test_release_automation.py, real script, fake host)

1. Implicit local placement installs two `/bin/sh <wrapper>` lines, and
   unrelated entries survive. Simulate cron: take the command field, cut it to
   999 bytes, and run it with `/bin/sh -c` and `env={}`. The fake gate records
   `--wave infrastructure --batch graph release-run`. The fake host PATH carries
   a padding component of more than 1,500 bytes and a directory with a space,
   which is the regression for the actual failure.
2. Explicit placement on this Machine is accepted, and placement elsewhere
   leaves the crontab unchanged (existing tests stay).
3. When the crontab write fails, the launchd plists are kept (existing test
   stays).
4. A repo path long enough to push the line past 999 bytes makes bootstrap exit
   non-zero with the crontab unchanged.

Focused check: `uv run pytest python/tests/test_release_automation.py -q` and
`bash -n scripts/bootstrap-cron-host.sh`. No Rust changes.

## Delivery and host cutover

1. Done: the branch was reset onto origin/main 2f329d7394 (old head kept as
   local branch `loo-629-pre-reset-backup`). Remaining: open a new repair PR and
   land it autonomously (Jack authorized this).
2. On ~/src/loopflow at main, run `scripts/bootstrap-cron-host.sh
   infrastructure`. The existing loopflow lines are replaced by tag, unrelated
   lines are preserved, and launchd is already retired, so the loop is a no-op.
   Keep the log under /tmp and record the result here in one line.
3. The telemetry step may still go red on `self doctor` lineage, which LOO-643
   owns. That shows the wrapper ran lf. It is not a failure of this fix.

## Done when

- Every installed `loopflow:infrastructure:*` crontab line is under 1,000 bytes
  and runs `/bin/sh` on a wrapper that holds the full gate-pinned command.
- Bootstrap's configured-path step ran the telemetry wrapper on this Mac and
  wrote `.lf/logs/telemetry-daily.log`.
- An actual scheduled receipt exists. Elapsed time alone is not proof:
  2026-10-28 09:00 creates `telemetry-daily.log` entries from cron, and 10:00
  creates `.lf/logs/release-run.log` with a publish or a no-change result.
  LOO-645 keeps both receipts and the later no-new-commits run. If the root
  publisher still owns publication at 10:00, record that deferral as it is.

## Exclusions

Telemetry lineage (LOO-643), v0.14.19 correction PR #1737 and its publisher
(Infrastructure primary conversation), scheduled-receipt bookkeeping (LOO-645),
credentials, and the live store.

## Delete — do not maintain

- The inline `cd … && env -i …` crontab command in `job()`. The wrapper replaces
  it, with no fallback to the inline form.
- The test assertion `" && env -i HOME=" in command` on the crontab line.
- `scratch/loo-629-verify.md` is folded into this plan.

## Evidence carried over

- #1732 cutover at 09:06: implicit placement and preflight passed, both lines
  were installed, and both launchd jobs were removed (/tmp/loo629-bootstrap-verify2.log).
- The 10:00 run failed through truncation (/var/mail/jack), and release-run.log
  is absent.

Check: `uv run pytest python/tests/test_release_automation.py -q` 15 passed; `bash -n scripts/bootstrap-cron-host.sh` ok. Host cutover (delivery step 2) and scheduled receipts remain.

Concept review (2026-10-27, headless): no change. One wrapper per job is the scheduled command and the bootstrap's proof; the 1,000-byte refusal stays as the loud form of a silent failure. Host crontab still holds the 1,430/1,419-byte lines, so the 2026-10-28 09:00 run fails until delivery step 2 runs.
