An interrupted terminal paste could leave Codex silently collecting later typing and Enter presses. Sessions now close the unfinished paste when their input view disconnects or loses control, releasing the accumulated draft without submitting it.

<!-- loopflow:task-pr-context:start -->
> [!NOTE]
> **Task:** [Keep interrupted terminal pastes from swallowing Session input · LOO-912](https://linear.app/loopflow/issue/LOO-912/keep-interrupted-terminal-pastes-from-swallowing-session-input)
> **PR lifecycle:** The pull request is published for review; no Task settlement is requested.
<!-- loopflow:task-pr-context:end -->

## What changes

- Track paste delimiters across input chunks while forwarding content immediately.
- Close unfinished pastes on view replacement, detachment, and holder writer loss. Restore framing around an unsent paste tail after reconnect, without replaying content or adding Return.
- Forward Ctrl+] inside pasted text; keep its detach behavior outside a paste. Advance the viewer and holder protocol revisions for these recovery semantics.

Existing holders remain alive across upgrades. Revision 1 holders retain their writer behavior; protection against a server disappearing mid-paste requires a revision 2 holder. View-control recovery applies to existing holders once the new server is tracking their input.

The live LOO-904 stall cleared immediately when its missing paste-end marker arrived. Its original keystroke stream was not recorded, so the exact transition that lost that marker remains unknown. Disposable PTY regressions cover the interrupted-paste paths repaired here.

## Validation

After integrating current main, 45 transport integration tests, the moved-Task notice test, and all 9 paste regressions passed locally. Five host-specific transport cases remained ignored locally; CI covers the Linux SSH cases. `cargo clippy --all-targets --jobs 4 -- -D warnings` and formatting passed. The integration entrance fixtures now pin viewer revision 3, retaining main’s typed close handling and the paste recovery semantics together.
