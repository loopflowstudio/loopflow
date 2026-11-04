# LOO-582 configured demo — November 4, 2026

## Current result: creation and binding preserve the live chat

Operator observations, not Jack Heart's acceptance or shipping approval.
Accepted design: [loo-582.md](loo-582.md). Interactive runbook: [demo.html](demo.html).
Candidate: `545b306be691c9e187a147f1a1224c8e1f2fcf98` plus this checkout's
workspace-projection and Desktop pane repair. Both the candidate CLI and the
embedded Rust library were rebuilt before the successful native-app replay.

Jack supplied a screenshot showing **Session location unavailable**, after the
initial demo had shown **Active in another terminal** and a layout error. The
repair removes the unplaced-primary special case that discarded an existing
Session workspace. Desktop relocates the Session pane to its permanent Task
while retaining its terminal identity, and removes the obsolete repository pane
reference. A remote physical Task identity does not redirect a local Work pane.
No extra cwd state, provider restart, input transfer or replay was introduced.

### Creation: passed

- Started configured Codex in the private repository conversation and established
  `LOO582-REPAIRED-618` plus a real `pwd` response in `prototype.repo`.
- Typed `UNSENT: repeat the remembered marker and run pwd. Keep all files unchanged.`
  without submitting it. Cmd-K → Bind → Create Task from this conversation created
  local Task `423c00` (Keep the visible draft).
- The same chat and complete unsent draft remained visible, with no layout error.
  The same Session, live agent process, terminal ID, cwd, workspace and source-file
  hash survived. The shared checkout was not adopted as a dedicated Task checkout.
- Submitted the retained draft. Codex recalled `LOO582-REPAIRED-618` and ran `pwd`
  in the original checkout. The native provider thread was unchanged.

Session `7b7e1d1bf7d8420a8824af30195c2d8c`; process
`6ac13c3d-fb16-41be-908a-e70ff3b2927c`; terminal
`c69a2120-01cc-4d36-a797-22fb8ca0f1cb`; native thread
`01a1a81a-5e23-78c1-b5ab-e8ebce2d9b8c`.

### Existing Task binding: passed within the accepted contract

- Created local Task `03ecd9` (Bind destination) and prepared its separate checkout.
  Established marker `LOO582-BIND-702` in another live repository conversation.
- Left `UNSENT BIND: repeat the remembered marker and run pwd without modifying files.`
  in the composer. From shell, ran explicit `lf session bind SESSION --task 03ecd9`
  and repeated it against the same target.
- Both receipts immediately selected `prototype.03ecd9-bind-destination`, made
  the Session primary and preserved its live process and terminal. No
  `requested_cwd` field was present. Desktop followed the Task and kept the draft.
- Submitted the draft; Codex recalled the marker. Its actual `pwd` remained
  `prototype.repo`, the accepted behavior for an already-running native client.
  Source `draft.txt` kept the same hash and was not copied into the destination.
  Native thread remained `01a1a81c-1673-7f43-8758-edf0d7a67d78`.

Binding Session: `2a36c48a0359485e958d0ed7052d8d4b`. The current-candidate replay
proves immediate selection and live continuity, not native-process relocation.
Reconnection into the selected directory retains focused automated coverage;
no provider exit or takeover was needed for this replay.

## App and reproducible evidence

Use **Loopflow LOO-582 Live** at
`/Users/jack/.lf/demos/loo582-0bubfe_n/repaired/Loopflow LOO-582 Live.app`.
The older Demo app and the original Live bundle are superseded. The original
viewer stayed alive without windows after Quit, so the rebuilt Live app uses a
fresh isolated bundle identity. The installed Loopflow app was not changed.

Persistent evidence root: `/Users/jack/.lf/demos/loo582-0bubfe_n`.

- `repair-{before,after,continued}.png`: successful creation and submitted draft.
- `repair-create-{before,after}.json`, `repair-history-{before,after}.json`:
  unchanged process/terminal/workspace and native thread receipts.
- `bind-current-{before,after,continued}.png`: successful explicit shell binding.
- `bind-current-{before,result,retry}.json`, `bind-current-history-{before,after}.json`:
  immediate cwd selection, same-target retry and native continuity.
- `repair-file-before.json`: source-file hash checked after creation and binding.
- `desktop-{before,after,location-unavailable}.png`: preserved original failure.
- `/tmp/loo582-continuity-{rust,swift,build,ffi}.log`: checks and native builds.

Focused validation passed: 13 Rust promotion tests, 16 Swift controls/workspace
tests, `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, CLI build,
FFI static-library build and relinked Desktop build. The combined gate and a
disposable installation upgrade remain outstanding. No real Task Graph was
restarted and no shipping approval was given. The next authored interaction is
review of this successful replay; the Task remains at `demo`.

## Human feedback and next action

Jack's screenshot establishes the broken experience before repair. His earlier
“i closed the app sorry” was a setup interruption, not acceptance. The repaired
screenshots and behavior above were observed autonomously by the operator.
No acceptance or additional design change is inferred. Review the Live app or
`demo.html`; retain the accepted simple binding contract and proceed to the
combined gate when this demo review is complete.

## Superseded failure: Desktop continuity before repair

Operator observations from candidate `545b306be691c9e187a147f1a1224c8e1f2fcf98`,
not Jack Heart's acceptance or shipping approval. The accepted contract remains
[loo-582.md](loo-582.md). The review page is [demo.html](demo.html).

The current CLI and Desktop were built locally and run against a private Machine
and local Git repository, using configured Codex. No fake provider, UI test mode,
shared installation upgrade or external demo Task was used.

1. Resumed the private repository conversation in Desktop. Codex remembered
   `LOO582-DESKTOP-517` and ran `pwd` in `prototype.repo`.
2. Typed, without submitting: `UNSENT draft: repeat the earlier continuity marker
   and run pwd. Do not edit files.` The before screenshot shows the whole draft.
3. Opened Cmd-K → Bind → Bind to Task…, entered `Keep prototype conversation`,
   and clicked **Create Task from this conversation**.
4. Creation succeeded as local Task `b7472a`. Session, live agent process,
   terminal ID and cwd were unchanged, and the Session became Task primary.
   The shared repository checkout remained in place, as intended.
5. Desktop replaced the conversation with **Active in another terminal** and
   **Layout not saved: invalid data: Session is unavailable in this repository**.
   The draft was no longer visible. A later observation showed the same failure.
   This fails continuous use in Desktop; it does not prove the draft was deleted.

The operator did not click Move here, transfer control, close the Session, submit
the hidden draft, or restart a Graph. Those actions would obscure continuity
and could lose input. The live fixture is retained for diagnosis.

Jack said “i closed the app sorry” during setup. The app was reopened before the
successful provider response and the recorded before/after sequence. That
interruption is neither acceptance nor evidence for the later continuity failure.
No new design change was requested or agreed in this demo.

Jack subsequently supplied a screenshot showing **Session location unavailable**
with the same layout error. The operator confirmed that state in the Live app
and began a targeted repair. `desktop-location-unavailable.png` preserves it.

## Recommended implementation and proof

Keep the existing attached terminal when the selected Session changes from
repository work to Task work. Reconcile pane ownership and saved layout with the
new Work address; do not add a second cwd state or repair by taking over the
provider. `WorkModel.createTaskFromSession` changes selection after refresh, and
`store/sqlite/selections/panes.rs::validate_sessions` rejects a repository layout
once its Session no longer has repository primary scope. This is a diagnostic
lead, not a verified complete root cause.

Reproduce and repair the repository-primary → new Task transition, then repeat
this exact Desktop sequence. Required proof: the visible unsent draft survives,
its submission recalls the earlier marker, and Session/provider identity stays
unchanged. Immediate bind into an existing different checkout remains unproven
on this candidate; perform it after continuity is repaired. The focused tests
previously passed but did not establish this native UI behavior. Combined gate
and disposable installation upgrade remain outstanding.

No Graph was restarted or shipping armed. Fresh status still reports `code` at
`demo`, PR #1807 open; no associated live Graph was observed. This is a failed
demo with concrete repair direction, not completion of the Task.

## Current evidence

Persistent private root: `/Users/jack/.lf/demos/loo582-0bubfe_n`.
Session: `653ba926994840c8aa29b9a9105ed20c`.
Agent process: `86d1d226-5f73-453e-8b12-1abb677d988e`.
Terminal: `f5a4b15c-3da9-45c7-8a9c-8cb8ce07bec0`.
Task: `task_b7472a33de8947e6bc57ef5d6c9c518b`.

- `desktop-before.png`, `desktop-after.png`: captured native app screenshots,
  also embedded in `demo.html`.
- `create-before.json`, `create-after.json`: configured CLI Session receipts.
- `create-history-after.json`: retained provider history receipts.
- `prepare-repo-session.json`: initial Session preparation.
- `/tmp/loo582-current-demo-build.log`: successful candidate CLI/Desktop build.
- `/tmp/loo582-demo-final-{status,ps,sessions}.json`: real Task final reads.

## Historical November 3 evidence

Earlier deferred-directory candidates below are superseded. They do not prove
this candidate's immediate-cwd behavior or Desktop composer continuity.

## Retained observations

- Candidate `1284e4b02726ffa11a7a778764fb80dc709f5445`, configured Codex and an
  isolated Desktop build: plain creation left dirty work unassigned; explicit
  `--session` adopted the same checkout and conversation; retry returned the
  same Task. Session, provider thread, live process, capture, cwd and file hashes
  survived. An unsent draft survived creation and Task progress; submission
  recalled `LOO582-RIVER-731` and ran pwd in the original checkout.
- Plain `--run` created separate Task 49181d, checkout and Session, stopping at
  the fixture graph's first interactive step. This proves admission, not
  autonomous implementation. Desktop's Task palette selected the adopted chat;
  explicit attachment transfer rendered retained history after draft submission.
- Candidate `057f6bc874`: bind/retry retained process, draft and source files.
  Submitting still ran in the source. Exit/resume retained native thread
  `01a1a565-b744-77e3-9f0a-b1e6758759a5`, recalled `LOO582-BIND-924`, and ran pwd
  in `demo.49181d-create-and-run`. That candidate cleared requested_cwd after
  confirmation; this is historical evidence, not the current binding contract.
- Rex and cmux Cmd-T opened the Session worktree. A later cmux failure with
  mixed-case hostname `Jacks-MacBook-Pro-2.local` exposed URL normalization.
  Preserving validated OS spelling repaired it; actual new-shell pwd matched.
  Eight output tests, fmt, Clippy and CLI build passed; log:
  `/tmp/loo582-cwd-fix-checks.log`. No terminal preferences changed.
- Closing only the cmux viewer left the provider detached for 139 seconds,
  beyond its 120-second grace. Reattachment preserved the exact unsent draft
  `UNSENT LOO582-IDLE-438: preserve this draft across the detached grace.`
  This proves protected-input retention, not expiry of an eligible settled owner.
  Ctrl+] did not detach; terminal-tab closure supplied that boundary.

## Failures and remaining proof

Warp access was refused by computer-use tooling. The isolated Desktop could not
open an exact taskless Session, and CLI launch of its custom bundle failed Launch
Services resolution; native Task palette navigation worked. Screens were inspected
but no persisted image path was supplied; a rendered Rex capture is retained.
These limits do not establish released-app defects.

Demo owns the immediate-directory candidate and Desktop create-from-chat with an
unsent composer draft. Native CLI draft retention does not establish Desktop
composer continuity. Combined gate and disposable installation upgrade follow
the authored demo boundary. No live mobile checkout or shared install changed.

## Evidence locations

Disposable root: `/private/tmp/loo582-demo-w9ra8f85`; `lf-demo` selects its private
`machine/` and candidate CLI using configured credentials without copying them.
Creation Session: `session_80872e1999814426861d7d5fffd90811`; native thread:
`01a1a473-54b3-7623-9aac-a1273467949c`; binding Session:
`session_4a2b7488715f472ba69b7f5b2f9f8e01`.

Receipts there include `plain-create.json`, `adopt-{task,before,after}.json`,
`continued.{txt,html}`, `create-run{,-status}.json`, `open-{session,adopted,desktop}.json`,
`binding-{result,retry,history-before,history-after,after-exit,after-resume,open}.json`
and `idle-{start,after-grace}.json`. Temporary files may expire. Full chronology
and receipt spellings: `b1427959fc347478a86449306e8ad49852d26574:scratch/loo-582-demo.md`.
