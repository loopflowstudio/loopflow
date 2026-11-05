# LOO-582 assumptions

Jack Heart's November 4 direction selects one Session cwd immediately on binding.
No product choice remains open; `loo-582.md` owns the accepted contract.

Existing connection paths handle provider cwd updates. OpenCode's native move
endpoint belongs to its connection; binding exposes no live provider-control
endpoint. Native clients may keep their launch directory, as Jack accepted.
No transport control, restart, draft transfer or interruption is introduced.

The missing-destination fixture belonged exclusively to the deleted pending-cwd
gate. Ordinary reconnection may create local launch scaffolding at selected cwd;
provider-move rejection and retry coverage remain applicable without that gate.

## Server boundary question — November 4, 2026

Jack Heart asked whether the server protocol should replace much of the Swift/C
binding layer. Inspection of LOO-593 PR #1813 found only roadmap reads/watches
and Session rename routed through ServerClient; other served operations still
call LoopflowLibrary. A complete Desktop server cutover should remove that bridge,
its callback/pointer machinery and Swift FFI/static-link setup once all consumers
move, while retaining Ghostty's separate native rendering bindings. This is a
recommendation for the existing server cutover, not authorization to duplicate it
inside LOO-582. This Task's continuity repair changed no FFI boundary and remains
at demo review. No Graph or separate Task was launched for this question.
