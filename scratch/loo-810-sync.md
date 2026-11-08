# LOO-810 sync

2026-11-08: runtime checkpoint and the current prose audit saved before sync.
First merge stopped on uncommitted docs; the prose checkpoint retained them.

`lf sync --manual` then `lf sync --continue` completed against main 1c757e9fe2028c0073414dc6826395931a5aa743.
Combined session.move and Session model/blocked fields from main with creation
receipts and provider transitions; retained the audit’s linked operation reference.
Branch remains local. Focused merged-behavior check follows in the implementation plan.
