# LOO-303 open questions

Jack Heart · 2026-09-30. Resolved and LOO-298 entries were dropped; git history keeps them.

- **What "attempt 2" means on the new model.** Jack's evening steer asked for
  several Run attempts at one step position, with one current attempt. LOO-298 has
  since removed the Run and replacement-attempt objects. Retries now belong to
  Session history, and loop passes are lenses over the FlowSession. The assumption:
  show the step's retries from the parent's Session/Flow history, and never count
  in Swift. Check that the parent exposes an ordered, exact projection before
  building node detail, the running line and the chip.
- **Taskless Flow orphans.** A taskless Flow's conversation may be unbindable under
  the parent's constraints. The assumption: keep it in the room and show the
  parent's reason.
- **Bind scope.** Bind affects later usage only. Earlier usage keeps its owner, as
  the parent settled. The room binds conversations, never a whole Flow.
- **Wave-only conversations.** They stay orphans. The parent bind accepts
  `--task` only, so no Wave target is offered.
- **Deferred (Jack).** Wave-page New session, where Sessions can launch from, and
  the waveless first run.
- **Unproven.** Installed cold and warm Task links, configured providers and drafts,
  live captures and Jack's verdict are all open. Preparing the demo candidate waits
  for LOO-298 to land.
