# How work trackers model hierarchy

Research for one infinitely nestable work node that projects onto Linear and
Asana. Read 2026-11-20. Each claim carries a source tag:

- **docs**: vendor help or developer docs
- **API schema**: Linear's published `schema.graphql`, Asana's published OpenAPI file
- **changelog**: vendor changelog or announcement
- **third-party**: community forum, user report, other site
- **unverified**: could not confirm; do not build on it without a test

Sources used most:

- Linear schema: https://raw.githubusercontent.com/linear/linear/master/packages/sdk/src/schema.graphql (master, fetched 2026-11-20)
- Linear sub-issues doc: https://linear.app/docs/parent-and-sub-issues
- Asana OpenAPI: https://raw.githubusercontent.com/Asana/openapi/master/defs/asana_oas.yaml (master, fetched 2026-11-20)
- Asana object hierarchy: https://developers.asana.com/docs/object-hierarchy

Two findings change the picture from "the basics":

1. **Asana subtasks now inherit their ancestors' projects, live.** Shipped May
   2026 in the app, exposed in the API August 2026. The old rule "subtasks do
   not inherit the parent's project" is still printed in the developer docs and
   is now only true of *direct* membership.
2. **Linear copies; Asana (now) reads live.** Linear writes the parent's team,
   project and priority onto a sub-issue once and never reconciles them. A
   sub-issue can sit in a different team and a different project from its
   parent, and the API says so.

## 1. Limits and relationships

### Linear

| Question | Answer | Tag | Source |
|---|---|---|---|
| Sub-issue depth | No limit documented anywhere. The schema's only structural rule on `parentId` is "An issue cannot be its own ancestor." | API schema; the absence of a cap is **unverified** | schema, `IssueUpdateInput.parentId` |
| Children per issue | No limit documented. `children` is a paginated connection. | **unverified** | schema, `Issue.children` |
| Parents per issue | One. `Issue.parent: Issue` is a single nullable field. | API schema | schema |
| Child in a different team than parent | Allowed. "Setting this makes the new issue a sub-issue; the parent can belong to another team if you can access it." Also "Sub-issues can be assigned to any team or member in the workspace, not just the parent issue's team." | API schema; docs | schema, `IssueCreateInput.parentId`; https://linear.app/docs/teams |
| Child in a different project than parent | Allowed. The archive doc lists "Sub-issues in another project are not closed" as a state, and a 2024 changelog line handles "sub-issues that are part of a different project from their parent issue". | docs; changelog | https://linear.app/docs/delete-archive-issues; https://linear.app/changelog/2024-07-25-priority-for-projects-and-micro-adjust |
| Projects per issue | One. "Issues can only be associated with one project at a time. A workaround would be to create sub-issues for the task, then assign each sub-issue to a different project." | docs | https://linear.app/docs/projects |
| Project must match team | "UUID or identifier of a project that includes the issue's team." A project lists several teams; "Issues in the project can belong to any of its teams". | API schema | schema, `IssueUpdateInput.projectId`, `Project.teams` |
| Project in project | Not possible. `Project` has no parent field; it has `convertedFromIssue` and milestones only. | API schema | schema |
| Project in initiatives | Many. "A project can belong to more than one initiative", but it "cannot be linked to both an initiative and one of its ancestors or descendants." | API schema | schema, `Project.initiatives`, `InitiativeToProject` |
| Sub-initiative depth | "Sub-initiatives let you nest Initiatives up to five levels deep in a tree-like structure." Enterprise only. Enforced: "Cycles and excessive nesting depth are prevented." | docs; API schema | https://linear.app/docs/sub-initiatives; schema, `InitiativeRelation` |
| Parents per initiative | **Sources conflict.** Docs: "Initiatives can have multiple parents." Schema: `Initiative.parentInitiative: Initiative` (singular, "The immediate parent initiative"), while the webhook payload has `parentInitiatives` (plural) and the link is an edge record, `InitiativeRelation`. | docs vs API schema | same |
| Sub-team depth | "up to 5 levels of nesting", Enterprise. `Team.parent: Team` is single. | docs; API schema | https://linear.app/docs/sub-teams |
| Milestones | Belong to one project; an issue has at most one `projectMilestone`. Removing the project removes the milestone ("Null removes the project and milestone"). No nesting. | API schema | schema |

Non-hierarchy relations, `enum IssueRelationType { blocks, duplicate, related, similar }` (API schema; `similar` is "deprecated"):

- **blocks**: directed, "the source blocks the target". Nothing stops the
  blocked issue from moving. "Once the blocking issue has been resolved, the
  blocked issue flag turns green and moves under Related" (docs, read through a
  search snippet of https://linear.app/docs/issue-relations; the page itself
  would not load). History entries are written on both issues "when the
  blocking issue is resolved or when it is re-opened" (changelog,
  https://linear.app/changelog/2020-03-11-issue-relations). What a *cancelled*
  blocker does is **unverified**.
- **related**: undirected. "Related links have no meaningful direction, so read
  both an issue's relations and inverseRelations" (API schema). Mentioning an
  issue in a description or comment creates one automatically (docs, same
  snippet).
- **duplicate**: the only relation with a side effect. "A duplicate relation
  closes the source as a duplicate of the target and can move linked customer
  requests to the target." The duplicate status is system-managed: "Moving to
  the reserved duplicate status requires an existing duplicate relation"
  (API schema).
- One relation per pair: "Creating a relation replaces a conflicting relation
  between the pair; an identical relation is reused" (API schema).
- **Project dependencies** are a separate type, `ProjectRelation`, anchored on
  start date, target date or a milestone. "Currently we only support a end ->
  start dependency" in the timeline (docs,
  https://linear.app/docs/project-dependencies). They draw lines and colour
  them; no date shifting is documented.

### Asana

| Question | Answer | Tag | Source |
|---|---|---|---|
| Subtask depth | "There can be up to 5 levels of subtasks below a task." The help centre says "you can only have a maximum of 5 levels of subtasks." Whether the API rejects a sixth level, and with what error, is not documented. A 2017 forum post reports six. | docs; enforcement **unverified** | object hierarchy doc; https://forum.asana.com/t/update-subtasks-documention-which-incorrectly-references-subtask-limit/1028823; https://forum.asana.com/t/subtask-inception/9387 |
| Children per task | No documented cap. Community list notes column sums stop working "Once the 31st subtask is added to a task". | third-party | https://forum.asana.com/t/list-of-technical-and-data-limitations-in-asana/236641 |
| Parents per task | One task parent: "one (and only one) of its parents is a task". `parent` is read-only on PUT; change it with `POST /tasks/{gid}/setParent`, which accepts `null`. | docs; API schema | object hierarchy doc; OpenAPI `setParentForTask` |
| Projects per task | Up to 20. "A task can have at most 20 projects multi-homed to it." A subtask can also be added to projects directly ("Tasks can be multi-homed as subtasks"). | API schema; docs | OpenAPI `addProjectForTask`; object hierarchy doc |
| Child in a different project than parent | Direct membership: yes, any. Inherited membership: the child is *also* in every project of every ancestor and cannot leave them (see section 2). | changelog; third-party for "cannot leave" | https://forum.asana.com/t/new-subtasks-now-expose-inherited-project-memberships-in-the-api/1151597 |
| Task in no project | Allowed. "Tasks can be orphaned and belong to no projects". | docs | object hierarchy doc |
| Project in project | Not possible. "Projects can only exist in a single organization or workspace and only belong to a single team." Sections are one level. | docs | object hierarchy doc |
| Portfolio nesting | "Portfolios are collections of projects (or other portfolios)." No depth limit documented. 1,500 items per portfolio (help FAQ, read through a search snippet). A project can be in many portfolios. | docs; depth **unverified** | object hierarchy doc; https://help.asana.com/s/article/portfolios-faq |
| Goals | A DAG, not a tree: a goal can have several parent goals (community list: "Limit of 4 parent goals connected to a goal"), and tasks, projects and portfolios can all be "supporting resources". | API schema for the shape; third-party for 4 | OpenAPI goal relationships; limits list above |
| Dependencies | "A task can have at most 30 dependents and dependencies combined". | API schema | OpenAPI `addDependenciesForTask` |

Non-hierarchy relations:

- **dependencies / dependents** ("blocked by" / "blocking"). Soft: a blocked
  task can still be completed (third-party,
  https://forum.asana.com/t/able-to-complete-task-in-timeline-view-even-though-blocked-by-incomplete-task/136731).
  Search exposes `is_blocked` ("tasks with incomplete dependencies") and
  `is_blocking` (API schema). Rules can trigger on "Dependent task is no longer
  waiting" (third-party). Date shifting is the automatic part: when both tasks
  have due dates and neither is complete, moving the precedent shifts
  dependents, with a buffer option (docs, read through a search snippet of
  https://help.asana.com/s/article/auto-shifting-dates-for-dependent-tasks; the
  page would not load). A 2025 user reports no shifting across projects
  (third-party, https://forum.asana.com/t/also-shift-due-dates-for-dependent-tasks-when-they-are-not-in-the-same-project/1092330).
- **No related or duplicate relation.** Asana has "merge duplicate tasks" in the
  app and task mentions; neither is a typed relation in the API (unverified
  beyond the absence of any such field in the OpenAPI file).
- **Subtask date rollup** is a job, not a live field: `rollupTask` "will
  asynchronously roll up the subtask dates for the subtask tree", and
  `has_subtasks_date_mismatch` reports "the latest computed rollup state"
  (API schema).

## 2. Copied versus live

### Linear: copied at creation, then independent

| Field | Behaviour | Tag | Source line |
|---|---|---|---|
| Team | Copied in the app. "Sub-issues inherit the parent issue's team, priority, and project." In the API the caller chooses the team; nothing is inherited. | docs; API schema | sub-issues doc, section titled "Copy properties" |
| Project | Copied, and conditional in the API: "If omitted for a sub-issue, the parent's project is used when it includes that team; null leaves the issue without a project." | API schema | `IssueCreateInput.projectId` |
| Priority | Copied. | docs | same |
| Cycle | "They may also inherit its cycle when created in an active status." | docs | same |
| Labels | "Labels are not inherited." | docs | same |
| Assignee | "The assignee is inherited if you are assigned to the parent, or if all existing sub-issues share the parent's assignee." | docs | same |
| Milestone, due date, estimate | Not mentioned; treat as not inherited. | unverified | |
| Status | Independent, each issue in its own team's workflow. Linked only by the two team settings below. | docs | |
| Shared access (the one live field) | Read from the parent. `inheritsSharedAccess`: "Whether this issue inherits shared access from its parent issue." "Issues that inherit sharing from a parent issue cannot be shared directly." A person given a shared issue "can view and update the issue and any of its sub-issue tree". | API schema; docs | schema; https://linear.app/docs/private-teams |
| Team-level settings | Live from parent *team* to sub-team, not issue to issue: sub-teams inherit cycles, optionally statuses and estimates; inherited records carry `inheritedFrom`. Un-nesting turns used ones into "independent copies". | docs; API schema | https://linear.app/docs/sub-teams |

What happens to children:

| Parent event | Children | Tag | Source |
|---|---|---|---|
| Existing issue gets a parent | "When setting a parent issue, the sub-issue is now automatically moved to a parent's project when it's possible". One-time write. | changelog | https://linear.app/changelog/2022-11-10-label-groups |
| Parent moved to another project (app) | Children in the parent's old project move with it; "sub-issues that are part of a different project from their parent issue will not be moved". The first half is implied by that line, not stated. | changelog | https://linear.app/changelog/2024-07-25-priority-for-projects-and-micro-adjust |
| Parent moved to another project (API `issueUpdate`) | Not documented. | unverified | |
| Parent moved to another team | Not documented. The moved issue "gives it a new team identifier and maps team-specific values, such as status and labels, to the destination team". Nothing says children follow, and cross-team parents are legal, so the likely answer is they stay. | API schema for the issue; children **unverified** | `IssueUpdateInput.teamId` |
| Parent marked done | Children closed only if the *team* setting is on. "When the parent issue is marked as done, all remaining sub-issues will also be marked as done." Field: `autoCloseChildIssues`. | docs; changelog; API schema | sub-issues doc; https://linear.app/changelog/2024-09-06-auto-close-parent-and-sub-issues |
| All children done | Parent closed only if the team setting is on. "When all sub-issues are marked as done, the parent issue will also be marked as done automatically." Field: `autoCloseParentIssues`. Both default off ("Optionally, configure"). | same | same |
| Parent cancelled | Docs say "done"; the schema says "closed" ("automatically close all sub-issues when a parent issue in this team is closed"). Whether cancel cascades, and to which status, is not stated. Which team's setting applies to a cross-team child is not stated either. | unverified | |
| Parent archived | "Archives an issue and its sub-issues". Auto-archive waits: an issue will not archive while "The parent issue is not closed" or "Sub-issues are not all closed". | API schema; docs | `issueArchive`; https://linear.app/docs/delete-archive-issues |
| Parent deleted | "Moves an issue to the trash and trashes its unarchived sub-issues that you can access." 30-day restore. | API schema | `issueDelete` |
| Parent restored | "Restores an archived issue and its sub-issues", and "a project association is removed if the project no longer includes the issue's team." | API schema | `issueUnarchive` |
| Parent converted to project | Tree is flattened. "the original issue and its sub-issues are added to the project as standalone issues. The original issue is renamed to indicate the conversion, and sub-issue relationships are removed." The project keeps `convertedFromIssue`. | docs; API schema | sub-issues doc |
| Sub-issue made top-level | "Remove parent"; `parentId: null`. Keeps its own team and project. | docs; API schema | same |
| Project progress | Counts every issue in the project flat, parents and children alike: "Completed issues contribute their full estimate and started issues contribute 25%." No parent-from-children rollup of estimate or progress is documented. | API schema | `Project.progress` |

### Asana: independent fields, live project context

| Field | Behaviour | Tag | Source line |
|---|---|---|---|
| Project (direct) | Not copied. "Subtasks do not inherit the projects of their parent tasks." `projects` and `memberships` "still return direct memberships only". | docs; changelog | object hierarchy doc; API announcement |
| Project (inherited) | Live. "a subtask inherits its ancestor tasks' project associations even when it has not been added to those projects directly." `effective_memberships` is "The union of direct and inherited memberships for this task"; each entry has `is_inherited`. "Inherited entries omit `section`, since an inherited association has no section placement." | changelog; API schema | https://forum.asana.com/t/new-subtasks-now-expose-inherited-project-memberships-in-the-api/1151597 (2026-08-10) |
| Custom fields | Live, from the whole chain. `custom_fields` "includes custom fields from projects the task belongs to directly and from projects inherited from its full ancestor chain." Before August 2026 it covered direct projects and the immediate parent only. The *values* are the subtask's own. | API schema; changelog | OpenAPI task schema; same announcement |
| Depth of inheritance | "This inheritance works for subtasks up to 5 levels deep." | changelog | https://forum.asana.com/t/subtasks-now-show-parent-project-and-fields-in-the-task-pane/1140273 (2026-05-08) |
| Where inheritance stops | Subtasks "don't yet appear in project views like Timeline, Calendar, or Dashboards" and "aren't picked up by Rules or reflected in My Tasks groupings based on inherited project." Search does match: `projects.any` returns tasks "directly in these projects or inherit them from an ancestor task." | changelog; API schema | same |
| Opting out | None documented. Users report an inherited project "can't be removed" from the subtask without removing the parent from the project. | third-party | https://forum.asana.com/t/recent-asana-update-subtasks-forced-into-main-project-causing-major-workflow-issues/1141033 |
| Assignee, due date, tags, followers | Not inherited; each subtask has its own. | unverified (the help article would not load; consistent with the API, where a subtask is a full task) | |
| Completion | Independent in both directions, and not a setting. Completing a parent leaves subtasks open; completing all subtasks leaves the parent open. Auto-completion is a standing feature request. | third-party | https://forum.asana.com/t/automatically-marking-a-parent-task-complete-when-all-subtasks-have-been-completed/76436 |
| Permissions | Project access flows down containers: portfolio to project is a per-link switch, `is_access_control_inherited`: "When true, the portfolio members gain access to the project." Whether inherited project membership now grants project members access to subtasks is not stated. | API schema; subtask access **unverified** | OpenAPI |

What happens to children:

| Parent event | Children | Tag | Source |
|---|---|---|---|
| Parent added to / removed from a project | Inherited membership follows, since it is computed. Direct memberships of the child are untouched. | changelog (follows from the definition; not stated as an event rule) | API announcement |
| Parent completed | Nothing. | third-party | above |
| Parent deleted | **Sources conflict.** Three community tests in one thread: subtasks deleted with the parent; subtasks orphaned; subtasks deleted "except those that are multihomed". A subtask with no direct project emits no webhook when its parent is deleted. | third-party | https://forum.asana.com/t/webhook-events-for-subtasks-deleted-with-parent-task/1109737 |
| Subtask given a new parent or `null` (`setParent`) | Becomes a subtask elsewhere or a top-level task. Whether direct project memberships survive the move is not stated in the endpoint description. | API schema; memberships **unverified** | OpenAPI `setParentForTask` |
| Task converted to project | "Subtasks will become project tasks." "The task description will become the project description." "Attachments and comments will remain with the original task (linked to in the project description) and will not transfer." | docs (2015 blog; current behaviour unverified) | https://asana.com/inside-asana/turn-any-task-into-a-project |
| Dates | Not live. Parent dates are reconciled to children by an explicit rollup job. | API schema | OpenAPI `rollupTask` |

## 3. Linear versus Asana

| | Linear | Asana |
|---|---|---|
| Shape under a project | Tree of issues, one parent, one project each | Tree of tasks, one task parent, up to 20 direct projects each |
| Depth | Undocumented for issues; 5 for teams and initiatives | 5 for subtasks (documented); undocumented for portfolios |
| Is a child a full item? | Yes: own team, status, assignee, cycle, project | Yes: own assignee, dates, completion, direct projects |
| Parent's container on the child | Copied once at creation or when the parent is set | Read live from every ancestor (since May 2026), on top of any direct membership |
| Child outside parent's container | Yes, by editing the copy | Extra projects yes; leaving the parent's project no |
| Child outside parent's team | Yes | n/a (teams own projects, not tasks) |
| Status coupling | Off by default; two per-team switches, either direction | None; not a setting |
| Parent moves container | Children in the same project follow; others stay (app) | Inherited membership follows automatically |
| Delete parent | Children trashed with it | Conflicting reports; multi-homed children may survive |
| Promote item to container | Issue to project; tree flattened, parent links removed | Task to project; subtasks become tasks, comments stay behind |
| Above projects | Initiatives: tree per docs, 5 deep, project in many initiatives | Portfolios: nestable, project in many; Goals: DAG |
| Blocking | Flag only; history entry and colour change when resolved | Flag plus optional due-date shifting; 30 edges per task |
| Other relations | related, duplicate (merges and closes) | none typed |
| Sharing | Live from parent issue down the tree | From containers (project, portfolio) |
| Rollup | Project progress counts issues flat | Dates by explicit job; dashboards "only count tasks, not subtasks" as of 2015 |

Product reasoning each company has published:

**Linear** argues for small flat units and named levels. The Linear Method:
"Scope issues to be as small as possible"; "Break down work into smaller parts
and create an issue for each one when possible"; "Projects should be called
projects" (docs, https://linear.app/method/introduction). The sub-issue doc
places sub-issues in a narrow band: "when a set of work is too large to be a
single issue but too small to be a project." The conceptual model gives each
level a different job rather than a different size: "teams own the workflows
those issues move through", "projects organize related issues around a
deliverable", "initiatives organize related projects around a broader goal"
(docs, https://linear.app/docs/conceptual-model). The one-project rule is stated
with its own workaround, splitting into sub-issues per project. No Linear
statement on *why* projects do not nest was found (unverified).

**Asana** argues against containers. "A container data model uses one-to-one
relationships to organize project work"; it "doesn't allow for dynamic projects
or cross-functional teamwork"; Asana built "a system flexible enough that any
piece of work can have a one-to-many relationship" (docs,
https://asana.com/resources/work-graph). The developer docs repeat it:
objects "can also be associated with one or more other objects". On subtasks
the company has been cooler than its own data model: "We do not recommend
making sub-subtasks" (docs, object hierarchy), and the 2015 conversion post
says "If you have 5-10 subtasks on a task it may be time to use a project
instead". The 2026 inheritance change reverses a long-standing position that
project membership is always explicit; the request thread it answers ran from
2017 (third-party, https://forum.asana.com/t/allow-to-automatically-add-subtasks-to-project/3018).
No founder interview on projects versus tasks versus subtasks was found
(unverified).

Reading the two together: Linear treats the parent link as a grouping hint
between otherwise equal issues, so copying is safe and divergence is allowed.
Asana treats a task as a node in a graph of memberships, so context is a set
that can grow by inheritance but that a child cannot shrink.

## 4. Other models

**Jira.** Typed levels. "By default, Jira supports three levels of hierarchy":
epic, standard, subtask, and "A subtask can't have any child work items" (docs).
Premium admins add levels above epic in Plans; no maximum is stated (docs). One
`Parent` field replaced Epic Link and Parent Link (docs). Single parent.
Subtasks must share the parent's project; cross-project parents work only
between company-managed projects (third-party). The numeric level ids -1/0/1
are unverified here.
https://support.atlassian.com/jira-cloud-administration/docs/what-are-issue-types/ ·
https://support.atlassian.com/jira-software-cloud/docs/configure-custom-hierarchy-levels-in-advanced-roadmaps/

**GitHub Issues.** Single recursive type, strict tree. "You can add up to 100
sub-issues per parent issue" and "create up to eight levels of nested
sub-issues" (docs). One parent, shown by the REST shape and `replace_parent`
(API schema). Cross-repository parents allowed within one owner: "The sub-issue
must belong to the same repository owner as the parent issue" (API schema).
Issue types are an organization-level label on the issue, independent of depth
(unverified here). Blocked-by / blocking shipped separately in August 2025
(changelog). Progress is stored pre-aggregated so lists do not walk the tree
(vendor blog). Tasklist blocks were retired in favour of sub-issues (changelog).
https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/adding-sub-issues ·
https://github.blog/engineering/architecture-optimization/introducing-sub-issues-enhancing-issue-management-on-github/

**Notion.** Two layers. Storage is one recursive type: "Everything you see in
Notion is a block", each with one parent pointer used for permissions. Blocks
once could have several parents; Notion removed that because "ambiguity is
unacceptable in a permissions system" (vendor blog). Sub-items are a database
self-relation (`Parent item` / `Sub-item`); no depth or count limit is
documented (docs). Dependencies are another relation with optional date
shifting (docs). Board, calendar and gallery views can only show parents; a
rollup cannot read another rollup, so totals over descendants need formulas
(docs; third-party).
https://www.notion.com/blog/data-model-behind-notion ·
https://www.notion.com/help/tasks-and-dependencies

**ClickUp.** Typed containers (workspace, space, folder, list) over a recursive
task. Nested subtasks default to three levels and can be raised to seven; up to
1,000 subtasks per task (docs, via search snippet; the page returned 403, and a
marketing page says "unlimited"). A task can be in several lists, but "the home
List determines the status and Custom Fields" (docs, via snippet). A subtask
added to a second list shows there as a top-level task. Parent-to-subtask field
inheritance is a long-open request (third-party).
https://help.clickup.com/hc/en-us/articles/6304431740055-Nested-subtasks

**Height.** Shut down 2025-09-24 (third-party). Subtasks were "a first-class
citizen, visible from the main list", with the same attributes as any task, and
a task could be on several lists (docs, via snippet; pages now 404). Unlimited
depth is unverified.
https://alternativeto.net/news/2025/3/height-project-management-tool-to-shut-down-by-september-2025/

**Shortcut.** Typed above the story (objective, epic, story; iterations beside
them). Sub-tasks arrived 2025-09-15 and are stories: with the feature off they
"will still be visible as individual Stories". They "inherit the Epic and Team
fields"; estimates roll up; an optional automation moves the parent from its
sub-tasks (docs; vendor blog). Old "Tasks" became checklists. Whether a sub-task
can have sub-tasks is not stated (unverified).
https://www.shortcut.com/help/stories/sub-tasks

**Basecamp.** Flat on purpose: project, list, to-do, one level of groups. "you
can't make a group inside a group — that would lead to an eventual,
over-organized mess"; "We set out to prevent over-organization and
sub-sub-sub-lists" (vendor blog).
https://signalvnoise.com/svn3/new-in-basecamp-3-to-do-groups/

**Things and OmniFocus.** Things has fixed levels: area, project, heading,
to-do, checklist; projects do not nest (docs; third-party). OmniFocus is the
recursive one: "action groups may be placed inside other action groups", folders
inside folders, no stated depth (docs). Each group is sequential or parallel,
which decides which children are *available*, and a per-group setting makes it
"complete when the last action within it is marked complete" (docs). The
distinctive idea: the parent carries an ordering policy for its children, not a
status rollup.
https://support.omnigroup.com/documentation/omnifocus/mac/4.3.3/en/the-outline

**Taskwarrior and org-mode.** Taskwarrior has no subtasks: `project` is an
attribute and blocking is a readiness condition (docs); `depends:` as a DAG and
dotted sub-projects are from memory (unverified). Org-mode is the pure outline.
With `org-enforce-todo-dependencies`, "Org blocks entries from changing state to
DONE while they have TODO children that are not DONE" (docs). `ORDERED` blocks
later siblings and is deliberately "local to the current entry, not inherited"
(docs). Progress cookies count direct children unless switched to recursive
(docs). Tags and some properties are inherited live down the outline.
https://orgmode.org/manual/TODO-dependencies.html ·
https://orgmode.org/manual/Breaking-Down-Tasks.html

**Phabricator / Phorge Maniphest: a DAG.** Subtask and blocker are one edge
type, so a task can have several parents; cycles are checked. A tree view was
rejected because "indentation can not show a graph without duplicating nodes".
The graph is drawn only under about 100 related tasks (vendor tracker).
https://secure.phabricator.com/T4788

**Azure DevOps: typed, configurable per process.** Epic, feature, requirement,
task, renamed by process. "You can't assign a work item more than one parent"
(docs, via snippet). Nesting a type inside itself is allowed and breaks the
tools: backlogs "don't support reordering for same-category hierarchies" (docs).
https://learn.microsoft.com/en-us/azure/devops/boards/backlogs/resolve-backlog-reorder-issues?view=azure-devops

**Workflowy and Tana: transclusion.** Workflowy mirrors have "no original and
no copy"; one node appears in several places. Tana references point at a node
that has exactly one owner (docs, via snippets). This is the outliner answer to
multi-homing: one owner for permissions and deletion, many appearances.
https://workflowy.com/help/mirrors · https://tana.inc/docs/nodes-and-references

### Who uses a single recursive type, and what they report

| Model | Products |
|---|---|
| One recursive type, tree | GitHub sub-issues, Linear issues, Asana tasks, OmniFocus groups, org-mode, Workflowy, Notion blocks and sub-items, ClickUp tasks, Height, Shortcut sub-tasks |
| Typed levels | Jira, Azure DevOps, Things, Basecamp; and the layers *above* the recursive type in Linear (team, initiative, project), Asana (portfolio, project, section), ClickUp, Shortcut |
| DAG for hierarchy | Maniphest, Asana goals; Linear initiatives if the docs are right |
| DAG for dependencies only | GitHub, Notion, Asana, Linear, Taskwarrior |

Every vendor with a recursive work item keeps one parent and most cap depth
(GitHub 8, ClickUp 7, Asana 5). None of them lets the recursive type also be
the container: Linear and Asana both *convert* an item into a project and
flatten the tree when they do.

Problems reported:

- **Views hide or flatten children.** Notion's board, calendar and gallery are
  parents-only (docs). Asana subtasks stayed out of project views for years and
  still miss Timeline, Calendar and Dashboards (changelog). A ClickUp subtask in
  a second list appears as a top-level task (docs).
- **Rollup has two meanings.** Org-mode needs a switch between direct and
  recursive counts (docs). Notion cannot roll up a rollup (third-party). Linear
  counts project progress flat. Asana runs date rollup as a job and exposes a
  mismatch flag (API schema).
- **Cost of walking the tree.** GitHub keeps an aggregate table (vendor blog).
  Maniphest stops drawing past about 100 nodes (vendor tracker). Asana's rate
  limit docs warn about deeply nested requests (docs,
  https://developers.asana.com/docs/rate-limits).
- **Inheritance surprises both ways.** Asana users asked for inherited projects
  for nine years, then objected when a subtask could no longer leave the
  parent's project (third-party). ClickUp's home list silently decides status
  and fields (docs). Linear's copies drift without notice.
- **Ordering.** Azure DevOps turns off backlog reordering when a type nests in
  itself (docs).
- **Multiple parents versus permissions.** Notion removed them (vendor blog);
  Maniphest kept them and lost the tree view (vendor tracker).

No primary source names "subtasks lost at depth" as a complaint; the closest is
a Height changelog title about "added context for flattened subtasks", which
would not load (unverified).

## Needs a live API test

Linear:

1. Is there any depth cap on `parentId` chains? Create 10, 20, 50 levels and read the error, if any.
2. Is there a cap on children per issue?
3. `issueUpdate(projectId)` on a parent: do children in the old project move? (The changelog line describes the app.)
4. `issueUpdate(teamId)` on a parent: do children move, stay, or lose their project when the project does not include the new team?
5. Cancelling a parent with `autoCloseChildIssues` on: do children cancel, complete, or stay? Same for parent auto-close when the last child is cancelled rather than done.
6. Cross-team parent and child: whose team setting governs auto-close?
7. Can an initiative really have two parents through `InitiativeRelation`? What does `parentInitiative` return then?
8. Does a `blocks` relation change state when the blocker is cancelled?
9. Does `projectId: null` on a sub-issue at creation stick, or does the app-side copy reassert?

Asana:

1. Does the API reject a sixth subtask level, on `POST /tasks/{gid}/subtasks` and on `setParent`? Exact status and message.
2. `setParent` on a task with direct project memberships: are they kept?
3. `DELETE` on a parent: are subtasks trashed, orphaned, or does it depend on direct membership? Which webhooks fire?
4. Can `removeProject` take an inherited project off a subtask? Expected no; confirm the error.
5. Do project webhooks and rules fire for a subtask that is only an inherited member?
6. Does inherited membership grant project members access to a subtask they could not see before?
7. `effective_memberships` at depth 5 with ancestors in different sections of one project: one entry or several?
8. Completing a parent through the API: any change to subtasks' `completed`?
9. Is there a depth limit on portfolio-in-portfolio, and a cycle check?
10. Does a 21st `addProject` fail, and do inherited projects count toward the 20?
