# Write the docs first

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
The branch includes LOO-498 checkpoint `3d61147152`; its live work remains separate.

Review: a configured-host walkthrough remains available. Native terminal file
presentation requires a local checkout; Desktop retains its remote Files
transport. LOO-498's native restoration and remote-host gaps remain with that Task.

Sources: [Mark Larah's user-docs-first pattern](https://blog.larah.me/write-the-docs-first/)
and [working backward from the finished experience](https://maven.com/articles/start-with-the-press-release-amazon-method).
Jack Heart explicitly chose user documentation, not a press release.

Checks: work-open tests passed (18; 2 opt-in native tests skipped), with the
taskless linked-worktree case rerun after its fix. Headless TaskFiles and Desktop
pane checks passed after updating and rerunning the action-catalog fixture.
Rust action DTO round-trip, prompt goldens, README/index sync, cargo fmt, stable
clippy all-targets and diff checks passed. Prior graph checks remain applicable.
