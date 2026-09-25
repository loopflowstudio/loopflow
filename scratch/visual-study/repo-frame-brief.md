# A: one repository frame, with data first

The human has accepted A as default and the Overview → Session with Context → Session zoom. The preceding bounded contribution just added zoom to mockups.html. Refine this SAME design. Own ONLY scratch/visual-study/mockups.html. Parent owns design notes, wrapper, verification and presentation. No commits, installs, PM, native source edits or delegation. Use uv run for any Python.

Latest exact human feedback:
- “A calmer desktop workspace is a good vibe for this project”
- “I dont know about the ELSEWHERE tag, i get waht youre going for but simplify for now”
- “something inbetween what we had before -- where the single repo case was optimized for and teh repo was put in the header and things were default filtered to within one repo”
- “but still making it seem like the header and the nav are part of one UI”
- “Search maybe goes at the bottom and teh data goes at the top, to connect the nav and the repo header”

Implement this in A while preserving its visual character and the new zoom behavior. B/C can remain earlier alternatives; any shared badge simplification may apply to them.

1. One connected application frame. Place the current repository in the app header, visibly related/aligned with its navigation beneath it. The header and sidebar should feel designed together (shared material, typography, alignment and uninterrupted grouping), not two separately branded boxes. No duplicate Loopflow labels, stacked brand rows or repeated repo root in the scoped outline. Repo scope is part of the app UI, not the outer study toolbar. Use your visual judgment within the existing cream/burgundy character.
2. Default to Loopflow repository only. Its Waves begin at the top of the navigation under the repo header; Tasks and Sessions remain beneath them. A repo switcher in the header can select Loopflow or Cube. For broader exploration, an explicit All repositories option is reasonable, but don't add a permanent second global tree. Use the existing outline with a scope parameter. No new domain hierarchy. Scope search and shown work to that repo; preserve each repo's selected Task/workspace/drafts on return. Cube has sample data and an honest sample read warning. Avoid a repo header saying Cube while showing Loopflow's Task.
3. Search belongs at the bottom of the sidebar, accessible and anchored while data scrolls above. Preserve / shortcut, exact selection and draft. Search should not separate repo heading from its content. Keep any status/footer quiet; avoid a diagnostic wall.
4. Remove HERE/ELSEWHERE location badges from conversation rows and pane headers (also tertiary “· here” label in pure zoom). Names should lead. Keep actual sample location state internally. Selecting an external conversation should show its existing explicit opening/transfer choice in the main area; never make it auto-transfer. The user asked to simplify presentation, not to change process ownership. No need to redesign that choice beyond readable neutral wording.
5. Preserve all three A zoom levels: same selected Session/draft; hiding navigation/context/companion changes only presentation; returning restores panes. Repository switching returns to Overview. An upcoming/no-Session Task can't fabricate a Session for zoom. Respect reduced motion, keyboard focus, Escape and visible accessible control names. Header scope stays intelligible in all depths without repeating large context in Session-only mode.

Do not change index.html or create a replacement page. Target actual iframe sizes ~1390x810 and 1050x710. Parent will capture with lf screenshot and test selection, repo switching/filtering, bottom search, zoom and draft retention. Finish by reporting controls/assumptions, leave changes uncommitted. No browser presentation or self-authored full test suite is needed.
