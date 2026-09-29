# Saved Flow discovery and bounded Desktop inventory

LOO-298 · Jack Heart · 2026-09-29.

Finish line: list and inspect saved FlowSessions without selecting execution or
loading unrelated captures. Desktop obtains bounded Session pages and preserves
selection, panes, terminal, draft and focus across partial or failed refresh.
SQL timings, a page DTO, or a list that discards off-page panes do not qualify.

## Implementation choices

- Reuse Flow's list/show commands with `--sessions` for saved history. This avoids
  reserving the proposal's new `sessions`/`inspect` template names. Templates keep
  their existing list/show meaning. Filters are `--for-task`/`--for-wave`, not
  global launch selectors. A plain ID continuation needs no cursor wrapper.
- Read the existing runtime `parent_id`; root Task selection stays distinct from
  an active child. Detail uses the captured graph, never today's catalog.
- Record a new taskless Flow's canonical repo before taking SQLite's write lock.
  Runtime children inherit recorded parent evidence. Import leaves unknown repo
  unknown; no lookup of an old cwd. Bound repository remains owned by Wave.
- Add explicit paged Session inventory, ordered by stable ID, with a next cursor.
  Preserve the existing alphabetical list for ordinary inspection. Desktop uses
  only pages. Rename cannot move an ID between pages. Pages are not a snapshot
  against concurrent insertion or changed filter membership.
- Reconcile each partial page into retained records; remove missing identities
  only after a complete successful enumeration. Failed/canceled/replaced refresh
  retains observations and panes. Records added locally during enumeration remain
  retained. Existing generation/repository fencing rejects late reads.

## Proof

Use the released proposal's store/public checks after adapting event identity,
runtime parentage and command names. Prove template collisions, corrupt unrelated
captures, preserved historical repo missingness, parent/managed distinctions,
unchanged Started and source-free detail. Cover populated forward/canonical schema.

For Desktop, pause between pages and inject a later-page failure while a selected
terminal holds input and focus; retain all of them. Repeat successful refresh,
rename between pages and stale-repository completion. Measure final bounded reads
end to end; distinguish warm samples from uncontrolled cold OS caches.

## Review and measured limits

The review kept template names usable, moved Git observation outside the write
transaction, and retained unknown imported repositories. Parent and managed-root
filters are independent. Desktop only prunes after all pages succeed; a page is
not a complete inventory. No new lifecycle or cursor object was introduced.

Public CLI measurements use a disposable store with 5,000 Sessions and 5,000
Flows, 100-row pages. Session first-process read was 789.90 ms; five warm reads
had median 744.49 ms. Flow median was 554.33 ms. OS caches were uncontrolled and
startup/query costs were not separated: these are bounded whole-command samples,
not cold-cache measurements or a latency-budget pass. Public template-name and
rename-between-page probes pass. Logs are `.lf/tmp/cut-i/discovery-*.log`.

The mounted terminal proof pauses/fails a later page and then completes a fresh
refresh while retaining the selected off-page Session, three surfaces, input
draft and focus. It uses Ghostty with a local cat process, not a configured agent.
The full Swift suite passes 291 tests. Hosted verification of this slice and
configured Desktop/provider acceptance remain separate.

The complete isolated materialized Rust suite passes 2,029 tests, 16 skipped,
none unrun, with fail-fast disabled. Formatting, all-target Clippy, migration
history, architecture and generated HTML checks pass. The preceding two fixture
setup failures and their repairs remain in [evidence](evidence.md).
