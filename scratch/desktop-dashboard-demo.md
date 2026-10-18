# Desktop Dashboard demo — October 18, 2026

Review of LOO-497 at `e08cbb102d236fb6046a39cb20b02b2f374e390e`, published as
[PR 1580](https://github.com/loopflowstudio/loopflow/pull/1580).
Current contract: [design.md](design.md); native ownership and proof boundaries:
[desktop-api-coordination.md](desktop-api-coordination.md).

## Experience selected

Ordinary Wave → named Dashboard selecting two Tasks → hide one occurrence →
adjust the remaining composition → enter the hidden Task and retain a file draft
→ return and restore the occurrence. Query membership and arrangement must remain
separate; ordinary navigation must select one destination. Cold shell attachment
is a separate LOO-416 proof, not implied by restored layout.

Jack Heart requested this demo. No new product feedback or acceptance has been
received during this attempt. Earlier prototype acceptance is historical and
does not establish native behavior.

## Preparation evidence

- Installed `lf help desktop` exposes the previous `list`/`split` commands.
  This checkout's helper with a private `LF_HOME` exposes `state`, `query`,
  `arrange`, `layout`, and Task shell tabs. Main-Machine forwarding must not be
  confused with exercising the candidate.
- Native app inventory shows the installed Loopflow and a separate LOO-489
  Console preview already running. Neither was replaced or terminated.
- Prepared `/tmp/loo-497-desktop-preview/LOO-497 Demo.app`, bundle identifier
  `com.loopflow.mac.loo497demo`, from the existing focused-check Swift build.
  Source executable SHA256:
  `97c3115a01dfaba68aed1b6f089efb814f661aaaef4d84f3370eb063bd7c17bf`.
  Candidate `target/debug/lf` SHA256:
  `0b8b50af51337c538efe04684ba23315656f1a31cdf8381024a6a4240f0e4d24`.
- Used the repository's [offline clone recipe](../docs/data-clones.md), with
  `/Users/jack/.lf` as source and `/tmp/loo-497-desktop-demo-20261018` as destination.
  No provider history home or account credential was supplied.
- First clone creation failed with SQLite extended error 6410 after copying the
  database. The SDK's `sqlite3.h` names this `SQLITE_IOERR_GETTEMPPATH`.
  Disposed the incomplete clone through `lf clone dispose`; retry sets `TMPDIR`
  and `SQLITE_TMPDIR` to the new clone's own `tmp` directory. This retains the
  existing sandbox policy. Retry output: `/tmp/loo-497-demo-clone-result.txt`.
- Source inspection identifies a separate demonstration limitation:
  `rust/loopflow/src/lf/commands/desktop.rs` targets bundle ID
  `com.loopflow.mac` for live state/control. A separate preview cannot establish
  candidate CLI-to-native round-trip parity through that target.

## Result: blocked before native opening

The temporary-directory retry passed the first failure and continued database
preparation, but exceeded the available disk budget before the clone became ready.
At the stop decision, free space was 13 GiB and the clone's WAL was 4.1 GiB.
The final measurement before disposal was 8,383,574,016 database bytes plus
6,862,407,992 WAL bytes, with `clone.json` still reporting `ready: false`.
The agent terminated only its owned clone creator (PID 33530); its subprocess
result was `-15`. `lf clone dispose` removed the incomplete clone and free space
returned to 34 GiB. Neither existing Desktop app was terminated or replaced.

The conversational stop update cited a 16 GiB reserve; that number was incorrect.
[TESTING.md](../TESTING.md) documents continuing above **32 GiB**. The observed
13 GiB was below that reserve either way. This was an agent resource decision,
not an automatic approval rejection or a request for product permission.

No candidate app was launched. No Dashboard creation, hide/restore, resizing,
draft continuity, provider attachment or restart behavior was observed. Native
acceptance remains unresolved. Existing focused tests in [design.md](design.md)
remain evidence only for their stated headless boundaries.

## Feedback and design disposition

Jack Heart supplied no new observations or acceptance during this demo attempt.
No UX design changes were agreed. The save-first arrangement behavior for
anonymous queries remains the existing implementation assumption in
[questions.md](questions.md), not an agreed design change.

The agent's implementation recommendation is to make the supported populated
preview path usable before repeating this demo:

1. Ensure clone creation selects a writable temporary directory inside its own
   sandbox, including SQLite migration/validation work. The initial error and the
   successful progression after setting both temp variables support this repair.
2. Establish and budget the clone's peak database/WAL/temp/history footprint on
   capable storage, or reduce that peak through the owning clone implementation.
   Do not weaken the isolation policy or use an unconfined database copy.
3. Provide a supported candidate Desktop/readback pairing. The current live
   control transport addresses the installed bundle, while this preview has a
   different bundle identity; source inspection is evidence of that routing
   limitation, not a failed native round-trip measurement.

These are proposed follow-up directions, not newly launched implementation work.
Proof for the next demo: ready clone → candidate native repository/Wave → saved
two-Task Dashboard → hide/resize/direct Task draft/restore → matching native
readback, with stable Task ownership and no execution caused by query selection.
Then obtain Jack Heart's actual feedback; inspect cold attachment separately.

Evidence paths: this note at
`/Users/jack/src/loopflow.loo-497-drive-desktop-through/scratch/desktop-dashboard-demo.md`,
the revised current design at
`/Users/jack/src/loopflow.loo-497-drive-desktop-through/scratch/design.md`, and the
retry exit log at `/tmp/loo-497-demo-clone-result.txt`. The prepared, unlaunched
app remains at `/tmp/loo-497-desktop-preview/LOO-497 Demo.app`; the data clone has
been disposed. Takeaway: preparation failed; native behavior and human acceptance
are still unproved. This demo did not advance a Workflow, restart a Flow, publish,
merge or complete the Task.
