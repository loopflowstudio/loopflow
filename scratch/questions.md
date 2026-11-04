# Decisions

2026-11-04: Jack Heart accepted recurring generated-output retention of four hours
normally, one hour below 64 GiB free, and fifteen minutes below 32 GiB free after
the rebuild-cost consequence was presented. This resolves the earlier policy
question and the stopped Graph's sole blocker. Code and tests already implement
the accepted windows. Receipt-reporting clarifications are applied; continue
publication and the authored demo/delivery boundaries without replaying finished
implementation. Policy acceptance does not authorize landing or a machine sweep.

2026-11-04: No separate design was supplied. The Task directive selects the existing
cleaner. The implementation uses a stdlib runner through the system interpreter
on macOS so launchd never waits for uv resolution or its cache lock. Developer
verification uses `uv run`; the stable shell entry point preserves the installed
job's path.

2026-11-04: Draft policy retains the shared 64/32 GiB pressure thresholds and uses
4-hour/1-hour/15-minute generated-output retention. Home/cache/Trash retention
and native Task pruning stay unchanged. New direct builds racing inspection
remain unsupported. Jack Heart's later 24-hour direction applied to the parent's
one-time sweep, not approval of these recurring thresholds. Policy acceptance
remains in `scratch/loo-722.md`.

2026-11-04: The earlier unavailable launchctl read left running state unknown.
Jack's 17:37 UTC Task comment supplies newer evidence: the parent stopped the
smoke run at monitor pruning, finished the one-time sweep and reloaded the job
idle. This supersedes the unknown state without proving a completed scheduled
cycle. Reconciliation made no machine cleanup or schedule changes.

2026-11-04: The iteration explicitly requests broader discovery. The reversible
choice is a source-tree walk capped at 30 seconds/100,000 entries, without
descending into generated roots, Git metadata or symlinks. Global DerivedData
projects contribute Build/Index.noindex separately so package source survives;
ModuleCache.noindex, SDKStatCaches.noindex and CompilationCache.noindex remain
whole candidates. Other global entries contribute only Build/Index.noindex;
unknown directories and package source survive later cycles without depending on
Git metadata. Recurring retention still awaits demo/review; this iteration changes
neither the parent-owned schedule nor machine contents.
