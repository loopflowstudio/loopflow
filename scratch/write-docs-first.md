# LOO-728 — integrate write-docs-first

Accepted integration — Jack Heart, 2026-11-04. Incorporate the existing source
at `5e6d3b1fc1ad23bb0fed51f4c0ad9815fed67648`, preserving its branch,
checkout, conversation and draft PR #1810. This Task stays stacked on LOO-498
(PR #1814), whose head was `da41d2031f3526df7fab53292b936dcccc699d9b`
and whose PR remained open at integration. The source merged without conflicts;
the parent's newer workspace guide and corrections remain intact.

Accepted direction — Jack Heart, 2026-11-03: exploration uses prototypes and
diagrams to choose a direction. A new write-docs step begins pursue, before
implementation. Review and editing remain available while implementation continues.

Jack Heart subsequently clarified: edit the actual documentation in place and
ask the current host to open the relevant files. No scratch draft or wrapper.
With no host, presentation does nothing. Jack Heart approved stacking on LOO-498,
whose shared Files tool supplies Desktop's native view and terminal editors.

Implementation choice: `lf open --file PATH`, repeatable, presents files without
opening a conversation or launching a host. Desktop reuses the scratch-follow
attention policy for any file; terminal hosts use LOO-498's editor policy.
Graph loops still return to implement, preserving saved documentation edits.
LOO-498's live work remains separate. Its restoration, native split, lifecycle
and remote-host obligations are not acceptance work for this integration.

Review: a configured-host walkthrough remains available. Native terminal file
presentation requires a local checkout; Desktop retains its remote Files
transport. LOO-498's native restoration and remote-host gaps remain with that Task.

Sources: [Mark Larah's user-docs-first pattern](https://blog.larah.me/write-the-docs-first/)
and [working backward from the finished experience](https://maven.com/articles/start-with-the-press-release-amazon-method).
Jack Heart explicitly chose user documentation, not a press release.

## Delete — do not maintain

The source extracts `task_tool::file_editor` from the inline Files branch; both
Task and taskless presentation use that policy. No competing Files store,
scratch documentation copy, host launcher or review approval step is added.
No further predecessor is identified. Preserve original files, active drafts,
existing panes, Session identity and the loop-back-to-implement behavior.

## Remaining work

- Gate: graph compilation/Task progression and `world_recipes`, prompt golden,
  Desktop action DTO, TaskFiles/Desktop pane checks, README/index sync and the
  affected static checks. Imported source results at `5e6d3b1` are historical
  evidence, not merged-tree proof. No migration is added by LOO-728.
- Publication and configured-host demo/review remain the code Graph's later
  steps. No parent landing or extra approval boundary is authorized here.
  The requested new Task conversation in Rex remains a caller navigation step.

Affected user docs: `README.md`, `docs/index.md`, `docs/orchestration.md`, and
`docs/surfaces.md`. The parent's `docs/workspaces.md` retains its explicit gaps.

Check: `cargo test -p loopflow --test integration work_open_tests::` built and ran: 16 passed, 2 host-response timeouts, 2 native cases ignored; exact isolated reruns of `cmux_opens_the_task_conversation_through_session_resume` and `rex_reopens_native_task_window_without_creating_a_session_or_replaying_tools` both passed (timeout cause unproved, no timeout/assertion changes). `cargo +stable fmt --all`, `cargo +stable clippy --all-targets -- -D warnings` (1.99.1), `git diff --check` and `lf context` passed. Gate owns broader checks; demo owns configured-host behavior.
