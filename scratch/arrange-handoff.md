# Arrange work handoff

Historical October 16 launch context, superseded by Jack Heart's October 17–18
integration and Dashboard decisions in `design.md` and
`native-console-integration.md`. The current implementation already supplies the
grammar, pinned discovery and retained-tree persistence discussed as open below;
this handoff grants no new launch or sibling-edit authority.

Jack Heart supplied the complete October 16, 2026 brief, requested filing its four Tasks, then explicitly requested launching navigation and session lifecycle in separate workspaces of the current cmux window. Jack subsequently also selected LOO-497 for launch in its own workspace in that window. The condensed brief is in `arrange-navigation-brief.md`; the complete original, including annotated reasoning and its dated survey, is preserved at `refs/loopflow/evidence/loo-497-kickoff:arrange-navigation-brief.md`. Check actual source paths/API names before relying on the survey.

## Owners

- LOO-489 (Product): Enable the multitask navigation experience. Existing Task and checkout retained; October 16 direction supersedes conflicting older dashboard scope. Owns click behavior, sidebar, breadcrumbs/title bar, derived tiles/queries, Work-keyed layout integration and native navigation acceptance.
- LOO-497 (Product): Drive Desktop through separate selection and arrangement APIs. Jack explicitly selected this Task for execution after the initial two launches. Owns the shared selection/arrangement contract, `lf desktop state --json` first, and headless `view --plan`. Navigation and API work are selected concurrently; agree the shared interface while preserving this Task's API/model ownership. Do not launch a second API driver.
- LOO-416 (Infrastructure): Keep Task Sessions seamless across switching and app restart. Existing Task broadened, checkout now prepared. Owns live-session retention, attachment/relaunch and measured switching/surface-capacity behavior. Coordinate layout persistence with LOO-489 and transport with LOO-415; do not duplicate their owners.
- LOO-498 (Product): Arrange cmux, Rex and Herder around Loopflow Work. Newly filed, not selected for execution by this launch. Consumes LOO-497's plan and materializes it through host APIs.

LOO-490's Wave multiplexer prototype remains design evidence in its existing checkout; do not start a competing production implementation there. LOO-453 retains readiness/events work; LOO-422/415 retain program-status transport. LOO-414 retains remote launch and first-start admission. No related Task was canceled, completed or silently absorbed.

## Boundaries for the launched conversations

Start useful work in each Task's existing checkout using the new brief. Preserve existing dirty prototypes and notes. Resolve settled direction from October 16 before older scratch; preserve older evidence rather than deleting it. Inspect fresh Task execution and avoid duplicate drivers. Keep consequential open questions in the Task conversation. Launch is not approval of an unresolved design or permission to merge.

Navigation and lifecycle may run concurrently in separate Task checkouts. Agree the shared layout/Session identity interface before editing overlapping owners. Navigation owns membership/presentation; lifecycle owns whether the same live Session survives. A tile projects primary Session (or explicit zoom), never ordinary focus. Each Task owns one layout; containers hold Work/query references.

The brief's detailed §1.4a supersedes the transcript's illustrative `add` command: membership changes by selection/query edits only. Work remains owned by current shared APIs, not by a newly inferred Maestro service. Earlier lifecycle direction forbids a shared resident process; the newer server/client outcome does not by itself select a new daemon architecture. Resolve that concrete ownership in design.

Unsettled: project crumb/grouping, inbox shape, pinned-query UI/grammar/sort, surface capacity, literal Dock icons, and Rex menu/title-bar inventory. Default to ordinary macOS window separation pending any literal Dock-icon requirement. Competitor claims in the brief are dated evidence, not current runtime proof.
