# v0.13.13

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.13 reduces interruptions when several pieces of work run at once. Linear checks are shared across work boundaries, optional Session naming gives way when the store is busy, and PR publication uses current Git ancestry when a saved Task base is stale. The through-line is keeping prompts and delivery moving while preserving pending writes and branch history.

## Check Linear when work needs it

Agents no longer poll Linear independently while idle. Changes are acquired before Flow steps, after agent turns, when showing planning and before Linear writes, reducing duplicate requests as concurrent work grows.

- Team checks share ten-second freshness across processes and repositories. Paginated acquisition advances its watermark only after successful ingestion.
- Desktop and `--watch` retain checks with a 30–240-second backoff. Teams without live Tasks make no acquisition requests; locally finished or abandoned Tasks are excluded.
- Rate-limit responses and exhausted quota headers establish one cooldown across a Home. After reset, the next supported boundary retries pending delivery; lost responses retain uncertainty.
- Successful mutation responses settle writes without follow-up reads. Foreground Git exchange remains in place.
- Comments on an idle Task become available at the next supported boundary without waking its Session.

## Keep optional naming out of the prompt's way

Native title callbacks now bound their store work so contention does not make optional naming wait through the hook's five-second timeout. When ownership cannot be established promptly, the title stays unchanged.

- Title callbacks run before ordinary command admission, with a 100 ms Process-recording wait and separate 100 ms bounds for ownership lock waits and history scans.
- Process history remains best-effort, without creating or migrating a store. Failed observation is no longer retried at exit.

## Publish from the branch's current history

`lf pr open` and `lf pr publish` resolve their comparison from current Git ancestry, so a stale saved base after a rebase or rewritten main no longer blocks valid branch work. Publication preserves existing commits, including inherited dependencies absent from upstream.

- Compare HEAD with the selected upstream's current merge-base and refresh root Tasks' saved bases without relying on reflogs.
- Stacked Tasks retain their integration forks until explicit sync. Sync after a parent squash preserves child edits without restoring files deleted on main.
- Ownership checks remain, as do refusals for empty ranges and unrelated histories. Ancestry errors are shorter and leave the branch intact.

## Operational notes

Linear cooldowns are shared within one Home; separate Homes remain independent. Linear writes still use unconditional updates, so an unseen concurrent edit can be overwritten between observation and mutation.

- For rollout measurements in loopflow, etude, kata and fantasia, restore `linear.team` with `planning.provider: linear` before collecting live request and quota evidence.
- Live Linear request counts, rejected-request quota accounting and installed pending-write recovery remain unverified. Confirm retained writes settle at a supported boundary after reset.
- Title-hook contention probes and disposable PR scenarios passed, but installed acceptance in Kata and Etude remains outstanding. The reported Kata timeout has no confirmed lock trace; broader slow-status and records-read reports remain unresolved.
- Recorded checks include focused network-isolated tests, formatting, Clippy and architecture checks. The supplied evidence does not establish a passing full hosted matrix for the combined release; repaired Linux PR coverage also remains CI-owned.

## Small changes

- Task-preservation coverage removes a clock-boundary assumption while retaining checks that planning edits preserve execution state.