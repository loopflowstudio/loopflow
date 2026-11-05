# Direct server writes in Desktop — November 4, 2026

Jack Heart requested new measurements and real Desktop operations after removing
per-write CLI startup. This records the contributing conversation's native proof;
performance results are in [server-performance/results.md](server-performance/results.md). The active LOO-593 Graph owns code, checks
and publication. No merge or Auto-Ship is authorized.

Design: [mobile-backend.md](mobile-backend.md), with accepted execution direction in
[server-write-direction.md](server-write-direction.md).

## Observed through the app

Using the rebuilt SwiftPM Desktop and a source-built direct-write helper in an
isolated live Machine, the operator:

1. Clicked New Task, entered **Verify direct server writes from Desktop**, and
   clicked Start. Desktop created Task `0a58e8`, prepared its separate worktree,
   opened its Codex conversation and showed the research interactive boundary.
2. Used Cmd-K → Rename Session → Save to name Session
   `dd0b836f0a77472ea63f887dcc3e5498` **Direct write verified**. Its pane menu and
   the CLI Session reading showed that name.
3. Opened the changed `.gitignore` in Desktop's Files pane, chose Edit, appended
   `# Desktop direct-save verified`, and saved. Desktop reported **Saved. Previous
   file retained in recovery.** The file on disk contains the marker.
4. Sent a bounded conversational prompt through the native terminal; Codex
   answered **DIRECT-WRITE-OK**. No provider replacement was needed.
5. Prepared a separate unstarted Task, `d632a7`, through the CLI because that
   is where this Desktop exposes Edit description. The actual edit and save
   were performed in Desktop. After navigating away and reopening the Task,
   it still displayed `DESKTOP-NOTES-593`. Read-only SQLite verification confirmed
   `tasks.issue_description` contains the same text. The first Task above was
   created entirely through Desktop; this second record is fixture setup.

## Telemetry and persisted receipts

All three edits are recorded by the same real server PID 31997, with distinct
request traces in the normal `logs/lf.log`:

- Session rename: `59756ef4-cae3-44bf-b6dc-dd6695587913`, code 0, server execution 18 ms.
- File save: `e8f4840a-f7e5-423a-a508-5b9a5d02dbdf`, code 0, server execution 198 ms.
- Description save: `a26c13a4-bb7d-46c9-8e23-763351da5c89`, code 0, server execution 1795 ms.

These single-operation server durations are observations, not benchmark medians
or UI latency measurements. Starting a Task still starts actual execution; the
ordinary edits above use direct server mutation dispatch.

Fixture root: `/Users/jack/.lf/demos/loo593-direct-ioi_wowv`.
App: `Loopflow Direct Writes.app` in that root, bundle `com.lf.demo593d`.
The app and server use the identical signed helper in its Contents/MacOS/lf.
Packaging metadata and hashes are in `/tmp/loo593-direct-demo.json`; the binary
identifies itself as `0.15.1+59287ba3e.dirty` (the direct-write working diff).
Swift app/tests rebuilt successfully in `/tmp/loo593-direct-swift-build.log`.

Screenshots in the fixture root: `before-create.png`, `task-created.png`,
`session-renamed.png`, `file-saved.png`, `conversation-response.png`,
`description-saved.png`. Persisted receipts: `task-created.json`,
`sessions-renamed.json`, `description-persisted.json`, `write-telemetry.log`.

## Failures and limits retained

The new Task's primary conversation automatically attempted its research Graph;
headless launches were refused because this private Machine has no connected
managed Codex account. Native conversation access worked with the configured
provider. The operator then explicitly told the fixture conversation not to run
more Graphs. No credentials were copied or authentication changed. Task creation,
conversation, edits and saves are proved; a completed research run is not.

CUA coordinate input could not address the copied app window. AX actions and the
app's View → Focus Terminal command worked; raising the window and submitting
with Ctrl-M delivered the visible draft once. Those tooling retries are not
additional write requests. The normal app Save result and persisted values were
checked independently.

The first attempted description read used the compact `task status --json`,
which does not include description. The persisted check then used the actual
SQLite columns (`issue_title`, `issue_description`) after schema inspection.
The initial wrong-column query changed nothing.

This is automated operator evidence, not Jack Heart's acceptance. Revalidate any
material implementation changes after this build; the final measurements and
Graph verification must identify their own build and limitations.

## Follow-through

The matched rename benchmark measured 87.38 → 10.77 ms median and
105.30 → 20.63 ms p95. Read medians were close, with worse read tail and a slower
single restart observation. The report preserves those limits. Final parity
and remote-owner/no-local-fallback checks passed. The existing Task Graph
resumed after capacity became available; sync completed at `ee4eee8dd8` and
`0e597c17a1`. Remaining validation and publication to PR #1813 belong to that
Graph; no merge is authorized. The earlier usage-limit
failures remain recorded in Task history.

The native bundle predates the final error-formatting, stop-person migration
and provider-admission cwd refinements. The exercised successful rename/save
path is unchanged; focused automated coverage owns those later changes. No
claim is made that screenshots exercise every line of the final branch.

Follow-on validation: the Graph's expanded provider-cwd parity case initially
failed comparing macOS `/private/var/...` with `/var/...` in
`/tmp/loo593-direct-parity.log`. This is a path-spelling assertion in the new
case, not a failed native save. Canonical-path comparison repaired it;
`/tmp/loo593-direct-parity-final.log` and the later dispatcher simplification
check `/tmp/loo593-compress-parity.log` both passed. These results precede the
#1818/#1820 merge and do not establish final-build native or upgrade acceptance.
