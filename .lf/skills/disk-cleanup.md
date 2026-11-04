Reclaim disk on this Machine across every repository under `~/src`, then
report what was freed and what was left alone.

Cron runs this hourly from the main checkout (`scripts/disk-cleanup.sh install`).
No one is watching: never ask, and never remove anything this skill does not
name below.

## 1. Run the mechanical cleanup

```bash
scripts/disk-cleanup.sh
```

It can take half an hour; run it in the background and wait for it to exit.
Whichever checkout or lf build launched it, this is a complete run: report
what it did, without caveats about how it was launched.
It logs to `~/Library/Logs/loopflow-disk-cleanup.log` and appends every
removal to `~/.lf/logs/disk.log`. "skipped: another cleanup holds" means a run
is in progress: report that and stop.

## 2. Report

The script does all the deleting, including idle build output in every
checkout once nothing has written to it for 4 hours. You only read
its output and the repositories, and report:

- one line per repository with what it freed, from the script's `freed` lines;
- free space before and after, from its first and `done:` lines;
- each linked worktree (`git worktree list`) still holding uncommitted or
  unpushed work (`git status --porcelain`, `git log @{u}..`);
- each checkout whose build output was kept, with the reason the script gave
  (`kept … (held open)` or `kept … (written in the last 4 hours)`);
- each failed step with its error.

## Demo runtimes

A disposable demo on this Machine puts its runtime root at
`~/.lf-machine/demos/<id>`: the copied app or CLI and generated output. Its durable
root (private Machine, provider history, fixture repository) sits at
`~/.lf-machine/demo-history/<id>`, outside `~/.lf-machine/demos`, because the marker
below covers the entire runtime root.

The demo writes `.disposable-coding-demo` at the top of the runtime root at release
(a person's answer or a headless run's end), never at creation, containing exactly
`disposable runtime; evidence stored elsewhere`. Writing it restarts the cleaner's
retention window, so a long review cannot age out while it is live. An unmarked root
is reported as retained. Reclaiming marked roots belongs to LOO-722's runner;
`scripts/disk-cleanup.sh` here does not read the marker.

## Never

- Remove anything yourself: no `rm`, no worktree, branch, source file or
  `~/.lf` change, no disk.log line. `lf wt prune` and the script own deletion.
- Touch a checkout with uncommitted or unpushed work, or any path a live
  process holds.
- Remove anything to make the numbers look better: an uncertain case is kept
  and reported.
