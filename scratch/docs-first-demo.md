# LOO-728 acceptance — November 4, 2026

Jack Heart accepted Desktop-only automatic presentation in primary-conversation
comment `50456e16-156c-424e-8fa1-67030496483f`. The earlier configured Rex
agent-route requirement is superseded for the workflow. Explicit terminal file
opening remains supported. No policy choice or terminal auto-opening proof now
blocks this Task.

## Remaining Desktop proof

The workflow uses `lf open --host desktop --file <original>` and reports paths
when Desktop is absent, unavailable or has no matching Files view. It starts no
host, editor, browser or conversation. Headless Files tests cover Markdown/HTML
rendering, active-draft preservation and repeated presentation; CLI integration
covers no Desktop from terminal contexts and preserves explicit native opening.
Current results belong to `write-docs-first.md`.

Demo/review owns original-file presentation in an already attached Desktop,
unsaved-edit preservation on repeat, saved correction readback, recovery and
conversation continuity. Headless model/view checks do not claim mounted or
configured acceptance. Source PR #1810, parent #1814 and LOO-728 #1817 remain
preserved. Parent delivery precedes supported installation containing its draft
migration; no permission to land the parent or bypass safeguards follows.

## Historical terminal evidence

Full observations and unpublished diagnosis are preserved at
`5d2f6f21eca663c16be3b8e3459d9304fc13c8ed:scratch/docs-first-demo.md`;
its earlier archive is Git blob `3c105e3f2c64cfdd313e0fd6c0303f1971a4e13d`.

- Configured agent presentation failed because the older owner answered
  `{"answers":1,"seq":0,"kind":"controlled","held":true}` without placement.
  The candidate refused stale startup placement. No editor opened, and no
  configured draft/readback/continuity proof was established.
- Direct development CLI opening, with agent provenance removed diagnostically,
  displayed the original docs. Jack Heart confirmed “ok it opened”. Repeating
  reused the editor. Later missing panes have no established cause; recovery
  bytes are retained. `docs-first-dev-demo.md`, screenshots and `demo.html`
  preserve that historical walkthrough, not the new automatic workflow.
- An isolated real-Rex probe with a candidate owner preserved the original-file
  editor, focus and unsaved correction on repeat, saved/read the correction,
  and recovered missing-file failure. It establishes bounded explicit-opening
  behavior, not configured installation or acceptance.
- Full Git history repaired the candidate's migration ordering. The old catalog
  at `c136addfa` rebuilt successfully but promotion remained `ahead_pending`
  with only `rex_workspaces` unmerged. No shared store was modified.

The parent `df3b5e810d3841a19bc5911c332fb42f23193d46` is now integrated,
including its coherent installer and revised draft schema. The old preflight
is historical evidence for the previous schema; no configured demo was repeated.
