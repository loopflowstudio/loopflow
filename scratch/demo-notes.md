# LOO-1063 Demo Notes

## 2026-11-23 · Live demo against isolated prod snapshot (`/tmp/lf-demo-1063`)

- Jack Heart: “whqt is that arrows button”
  - Context: The icon-only `arrow.up.arrow.down` button (`Sort by status`) placed beside `+ New Task ⌘N` in `WorkNavigator.swift:79-86`. It was not immediately recognizable on sight.
- Jack Heart: “well, it needs a on/off state, and maybe an on-hover”
  - Context: Button currently uses `.buttonStyle(.plain)` with no hover treatment and no on/off visual state (it triggers a stateless one-shot write to `task_orders`).
- Jack Heart: “1a”
  - Context: Chose toggle view — **On** shows the sidebar sorted by status without overwriting saved drags; **Off** restores the Start-Time Stack / saved drag order. Plus hover highlight and clear hover feedback on the button.
- Jack Heart: “build it and return when ready”
  - Agent choice recorded for review: while toggled **On**, hold the currently selected Task's spot until selection changes (`2a`) so sending a prompt still never jumps the open Task; disable drag-to-reorder while status sort is toggled On; delete the superseded one-shot `sortWorkingTasksByStatus()` / `desktop.view.sort` mutation path if nothing else uses it.

## 2026-11-24 · Live demo of toggle build (`43f6d7fb48`) against isolated prod snapshot (`/tmp/lf-demo-1063`)

- Jack Heart: “lg2m ship it”
