# LOO-717 — Session viewer and shell lifetime demo

## November 6, 2026 — headless decision pass 1

**Advance.** Reviewed candidate `5f4ee78ee99a959c1cee251f884dc16189e520a3`
against the [accepted design](loo-717-shell-lifetimes.md) and
[decisions](questions.md). This is an autonomous delivery judgment, not a claim
that Jack tried the native experience. Jack Heart's recorded November 6 words,
“ok, lg2m, ship it”, authorize LOO-717 / PR #1811 with native and physical-remote
demonstration limits carried forward. The November 5 no-shipping statement below
is superseded for this Task only. LOO-498 / PR #1814 remains separate.

### Intended progress and observed results

The preceding pass was to publish the Desktop ownership revision with affected
checks, then return to demo. It removed the Task shell API and extra SSH terminal
layer, kept one Desktop execution owner, and repaired return-before-sweep
cancellation. Existing execution logs were inspected directly in this pass;
they were not rerun or presented as newly performed demonstrations.

| Experience | Observed result and source |
| --- | --- |
| Local reconnect retains state; Close settles jobs | Publication's real transport checks passed `desktop_shell_reconnects_without_replay_and_close_settles_its_jobs` and `desktop_shell_twenty_lifecycles_leave_no_owned_jobs_or_sockets`. |
| Remote reconnect uses the same execution without replay | Three real loopback OpenSSH checks passed in 119.29 seconds. The twenty-cycle check asserts destination cwd and Machine, same shell PID and variable, one startup per identity, Close/job settlement, owner socket removal and process/PTY/SSH baseline. This uses production Machine routing and the Desktop runtime with isolated stores, not native Desktop rendering or a physical peer. |
| Remote errors stay remote | The SSH failure check passed for rejected authentication, missing checkout and ended execution; it asserts no local shell creation. |
| Session return prevents pending automatic Close | The real transport regression `returning_task_viewer_cancels_shell_close_without_another_sweep` passed. Store/retirement checks passed for cancellation, observed absence deadlines, drafts/work protection, retries and interrupted finite sweeps. |
| The shared viewer protocol replaces the extra relay | Prepared SSH viewer reuse/leave and silent-viewer heartbeat checks passed. They do not establish Rex's own connection-loss detection. |
| Removed commands and private runtime metadata | Fresh built-CLI inspection in this pass rejected `help task shell` and `help session remote` with unknown-command errors. `help desktop shell` describes GUI-independent execution; its JSON reports a null result schema. |

Supporting logs: [471 selected Rust checks](../.lf/tmp/gate/run-94140/rust/rust.log),
`/tmp/loo717-publication-ssh.log`, `/tmp/loo717-publication-boundaries.log`,
`/tmp/loo717-publication-current.log`, and `/tmp/loo717-publication-upgrade.log`.
The latter records a populated Linux 0.14.3 upgrade retaining Task/Session identity,
history and edits. The aggregate records 687 passing headless Swift tests and an
initial architecture-map failure; the design records the correction and passing
recheck. There was no fresh all-green aggregate. These results add concrete
runtime coverage beyond the November 5 unlaunched private app, not native proof.

### Limits, feedback and next action

No new human observation or design change was made. No product question is open.
This surface has no rendering environment, so native Desktop/Rex interaction,
orderly GUI Quit/relaunch, actual two-Machine outages, sleep/logout/reboot and
wall-clock fifteen-minute cleanup plus scheduler cadence remain unproved.
The old private app below was not rebuilt or launched. Accelerated lifecycle
fixtures do not establish elapsed fifteen-minute behavior. The recorded Rex
opening failure in [coordination](rex-coordination.md) remains a separate gap.

No concrete implementation defect was found that warrants returning to implement.
Advance under the recorded shipping direction, retaining these limits in delivery
copy; the primary Task operator owns shipping and merge/completion verification.
This pass neither ships nor completes the Task, installs a candidate, registers a
schedule, or migrates the live store. The earlier native runbook remains available
below, but its absent observations are not conditions newly imposed on shipping.

No provider, shell owner, SSH daemon, app or background command was launched by this
pass. No draft or request was submitted; its own inspection input is settled.
The existing private history/runtime roots remain untouched. No new HTML page was
generated because this pass is headless; the November 5 page is not a current
demonstration result.

## November 5 preparation — historical, no app launched

November 5, 2026. Jack Heart invoked demo for the published change at
`500425244c`. Design: [shell lifetimes](loo-717-shell-lifetimes.md).
Review page: `scratch/demo.html` (written after the demonstration record).

## Scope and decision

Review the actual candidate's Desktop shell reconnect/Close and LF Task Session
viewer retention. No acceptance, shipping or installation has been given in this
review. Published automated passes remain separate from rendered demonstration.

The old candidate at `500425244c` retains unknown remote viewer loss indefinitely.
Jack subsequently accepted bounded disconnect cleanup and explicit Quit release;
the prior repair at `a1067c99cc` passed focused Rust checks, loopback SSH,
Clippy and the Swift build. Jack's November 6 revision replaces its extra SSH
terminal with the shared viewer protocol; its checks are in the current design.
The private copied app remains the old candidate and needs rebuilding before
demonstrating this revision. It does not include LOO-672's later automatic
conversation opening or LOO-498's layout launcher; native Rex can launch the demo
viewer directly.

## Private roots and provenance

- Durable root: `/Users/jack/.lf/demos/loo717-500425244c/durable`
- Runtime root: `/Users/jack/.lf/demos/loo717-500425244c/runtime`
- Private Machine: durable root `/machine` (`LF_HOME`).
- Candidate: runtime root `/Loopflow LOO-717 Demo.app` (whole copied bundle).
- Copied CLI: candidate `/Contents/MacOS/lf` (`LF_BIN` for all fixture operations).
- Fixture repository and local bare origin live under the durable root.
- Fixture uses local Git and disposable Task records, not a GitHub publication.
  Any seeded merged-delivery record is fixture setup, not a live PR merge proof.
- Rust CLI/FFI and Swift builds completed November 5. Jack then invoked
  concept-review; private fixture preparation and app/provider launch have not
  run. The review found consistency repairs in the working design. Exact
  Session/owner identities, screenshot paths,
  observations and commands will be recorded below before review handoff.

## Input and teardown

No user input or demo provider is launched yet. Preserve both roots while owners
live or review input is unresolved. Never delete durable history. End only verified
demo owners after review input is settled; an edited draft needs Jack's explicit
release in this conversation. Closing a viewer is not that release. The page will
ask for that release separately from behavioral acceptance.

## Runbook

1. Open the disposable Task's Desktop shell. Set a variable and note its PID;
   hide/switch/reconnect. Expect the same PID and variable. Close should end that
   shell/jobs; Undo should not restart them.
2. View the disposable Task Session in Rex, with an unrelated shell in its
   checkout. Complete only this fixture Task. Expect the LF viewer to retain the
   checkout even when unfocused. A second passive viewer also retains it.
3. Close/detach the last LF viewer. Expect ordinary fifteen-minute eligibility;
   the unrelated shell must neither retain the checkout nor receive LF Close.
   Any accelerated deadline must be labeled explicitly, not reported as a
   fifteen-minute timing proof.
4. Observe active-work protection and bounded remote disconnect cleanup where configured proof is
   available; carry absent physical remote-Machine evidence as unproved.
