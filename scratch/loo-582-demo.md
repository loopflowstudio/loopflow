# LOO-582 autonomous demo — November 3, 2026

Jack Heart requested an autonomous demo. The operator exercised the candidate
CLI and a separately packaged Desktop build with the configured Codex account,
using a disposable repository and private Loopflow store. These are operator
observations, not Jack's acceptance or authorization to ship.

## Observed

- Plain `task create` from a live, dirty linked checkout created Task 725ce5
  without adopting the checkout or binding its active conversation.
- Explicit `task create --session session_80872e1999814426861d7d5fffd90811`
  created 5951ba around the existing chat and checkout. Retrying returned 5951ba.
  Before/after receipts preserve the same Session ID, provider thread, live
  agent process, capture and cwd. README and untracked draft.txt hashes match.
- An unsent question remained in the native Codex composer through creation
  and a subsequent automatic Task progress message. After submission the same
  native thread remembered LOO582-RIVER-731 and ran pwd in the original checkout.
- Rex Cmd-T from that conversation opened a shell whose actual pwd was the
  linked checkout, while the launching shell had been in the parent repository.
- `session resume` continued the same conversation in cmux. Cmd-T there also
  opened the linked checkout; actual pwd matched. No terminal preferences changed.
- Desktop's Task palette opened 5951ba with the selected Session and correct
  checkout. It identified the attached terminal accurately. After the draft had
  been submitted, Move session here displayed the retained conversation/history
  in Desktop. A native screenshot was inspected in the conversation.
- Plain `task create --run` created separate Task 49181d, a distinct checkout
  demo.49181d-create-and-run and a new Session. It paused at the fixture graph's
  first interactive demo step, without adoption or Auto-Ship. This proves
  placement/admission, not autonomous implementation execution.

## Failures and remaining proof

- Warp: computer-use access was refused for safety reasons. No Warp result.
- Exact taskless Session open in the private Desktop build returned “Desktop
  opens exact Sessions through their Task.” After adoption the Task palette
  opened the selected Session normally.
- CLI open targeting the custom demo bundle could not resolve its unique bundle
  through Launch Services. Native palette navigation worked. This isolated
  packaging limitation is not evidence of a released-app navigation defect.
- Before/after screenshots were observed in the tool conversation, but the
  supported capture interface did not provide a persisted image path; no invented
  screenshot file is linked from the runbook. Rex rendered capture is retained.
- Binding into an existing Task and actual provider cwd after that transition
  still require the revised command's real-path demo. Desktop create-from-chat
  composer preservation and detached idle expiry are not yet established.
  An attached draft surviving several minutes is not proof of detached expiry.
- Full combined gate and disposable installation upgrade remain pending.

## Accepted feedback and current state

During this demo, Jack Heart rejected `task promote TASK`, then resolved the
replacement: `session bind SESSION --task TASK` always makes it primary and
requests continuation in the Task checkout. No optional-primary mode. Creation
around existing work stays explicit. The correction is implemented in this Task;
publication to PR #1807 and the revised binding demonstration remain pending.
See the current design in `loo-582.md`; presentation is `demo.html` and
`concept-review.html`. No Graph was restarted merely by performing the demo.

## Reproduction and evidence

Candidate commit: 1284e4b02726ffa11a7a778764fb80dc709f5445.
Disposable root: /private/tmp/loo582-demo-w9ra8f85.
Wrapper `lf-demo` selects private `machine/` and the candidate CLI; it uses the
configured provider without copying credentials. Source: demo.prototype.
Native Codex thread: 01a1a473-54b3-7623-9aac-a1273467949c.

Receipts in that root: plain-create.json, adopt-task.json, adopt-before.json,
adopt-after.json, continued.txt, continued.html, create-run.json,
create-run-status.json, open-session.json, open-adopted.json, open-desktop.json.
The files are local temporary evidence and may expire. No live mobile checkout
or shared application installation was modified.
