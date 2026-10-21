# Layout editing direction — October 16, 2026

Historical design input. Jack Heart's October 17–18 corrections in `design.md`
restrict compositions to Dashboards and preserve ordinary Console destinations.
The default-selection rules below describe shared plans, not native repo/Wave UI.
The lfdb mechanism stands; the retained split tree below was replaced on October 21
by ordered occurrences plus the slot layout (concept review in `design.md`). Remaining
acceptance and the unsaved-arrangement assumption live in `design.md`.

Jack Heart proposed representing arrangement as `v-split A, B, percentage` or `h-split A, B, percentage`. Hiding one side collapses the visible split to its sibling; showing the member again should fit this same model. Jack explicitly allowed changing the layout representation and suggested replacing a complete layout map as a viable baseline, while preferring simpler natural edits where useful. This refines the API design, not approval of the unfinished overall implementation plan.

Proposed interpretation for the design: keep a retained binary split tree with member-reference leaves, axis, ordered children and ratio. Hidden member IDs remain selection members; derive the visible tree by pruning hidden leaves and collapsing unary splits. Re-show uses the retained tree, recovering placement and ratio rather than reconstructing ownership. Full-tree replacement can be the complete arrangement operation, preserving the selection member multiset and repository boundary; move/swap/resize are conveniences using that same engine. Decide how omitted hidden members and duplicate projections are represented without creating a second tree authority.

Work through this case explicitly: hide A in split(A, B), edit B's arrangement while A is hidden, then show A. Restoration must preserve the newer edits, not restore an obsolete whole-tree snapshot. Define full-map edits over the retained tree (including hidden members) so the rule is predictable. Investigate existing LayoutNode.visible(excluding:) before inventing another hide/restore store.

## Repeated Tasks in nested queries

Jack Heart resolved duplicate presentation in the same conversation: “i think just first one wins i guess”. Implement first visible occurrence in deterministic layout traversal order (first child, then second), scoped to the repository window's displayed composition. That occurrence gets the Task's live tile; later occurrences of the same canonical Work key are deduplicated in presentation, not removed from their queries. Focus must not select the winner. Hiding/removing the winning occurrence promotes the next visible one; showing it again uses current layout order. State should distinguish explicit hide from duplicate suppression and identify the winning occurrence. Preserve one Task layout, Session identity and native surface. The visible-order/promotion details are implementation interpretation of Jack's first-wins decision.

## Persistence authority

Jack Heart explicitly authorized updating lfdb schemas as necessary for ideal persistence. Use shared lfdb records for durable query definitions and Work/query-keyed arrangement intent, consumed by both Desktop and headless plans; schema evolution is authorized and is no longer an unresolved permission question. Follow the repository's one migration draft per Task rule and released-frontier tests. Coordinate the concrete record shape/writers with LOO-489/416 rather than treating existing schema limitations as fixed. Preserve the separation between durable layout intent and native surface/attachment/process lifetime; do not persist native handles or infer liveness from a saved layout. The exact storage/transport implementation remains an engineering choice to resolve in the design, not an additional product approval gate.

## Default selection — design review, October 16, 2026

Jack Heart selected “started unfinished tasks. repo is that + waves.” He then
specified future subwaves: “subwaves + all open tasks eiter directly or in a
subwave”. Each container therefore shows its child Waves and all started,
unfinished Tasks anywhere beneath it. At repo level this includes the repo's
Waves and all its open Tasks; a Wave includes direct and descendant open Tasks.
Paused, waiting and idle started Tasks remain; unstarted backlog and finished
Tasks are absent from defaults. Waves do not require an open Task to appear.
First-wins presentation still deduplicates overlapping occurrences without
changing selection membership. Immediate-child Wave traversal is the working
interpretation; recursive Task membership is explicit accepted direction.
Subwave creation/UI is not newly selected work.
