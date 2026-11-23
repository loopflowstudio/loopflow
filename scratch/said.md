# Subwaves redesign: what was said, observed and tried

The running record, in order. The design is `scratch/plan.md`; this file holds the exact words, what the code showed, and drafts since revised.

## What was said (Jack Heart, 2026-11-18)

> Okay I want to start redesigning the idea of subwaves. I had previously conceived of these as continuing to be long-lived persistent waves but I want to open up the space between waves and tasks and build out something more like a subtask (but from the perspective of a subwave rather than a task, because a subwave doesn't necessarily need to have its own PR) but the idea is that it should be natural that, as you start doing some feature, you might find, "Oh actually this is a lot of work and we should branch it out into a lot of different things." Then you might continue to build on that work.
>
> It should be easier and I want to institutionalize the pattern of having a single session that manages that group of tasks (that evolved organically out of some task and follow-ups that you wanted to do with it, or realizing that you were in over your head and need to decompose it). You can look at, for example, the ship decomposed skill as one of the ways that stuff like this can happen. You can also look at: there have basically been two subwaves recently. One is the perf work and the other is the store work these are mostly projects that have a finite end to them although they don't necessarily have to be. We want to scale that up
>
> One place where things get tricky is that it's not clear to me that you necessarily would want to make a whole project for these. I think in this case we basically want to institutionalize the idea that you can just kind of defer to your parent for this project. It may even be that your parent has no projects but if you think it is long-lived, then you would want to be able to tie it to a linear project if you're using Linear, for example and I think, basically, if any wave has a project then that's the project. If you don't, you kind of climb up the tree until you find one and if you never find one then you just don't use a project
>
> I think this is the time where repos really become full-fledged waves. Maybe we still have a default repo session but I think we can sort of make repos more architecturally, truthfully, waves. I've been treating them as sort of basically implicitly the parent wave of every other wave. What's Start to make it truly that case?
> From a user experience perspective these need to be lightweight and instantly nestable. We need to think about how you might view things if you want to.
>
> I think at the sideboard level you can imagine that maybe we start having the notion of opening, having a task that's open or a session that's open for you. You could have certain waves. Sessions may or may not be open and by default you don't show tasks that are not open. Those may be the ones that are working on auto mode.
>
> Although it's interesting to think about: is there somewhere in between auto mode and not auto mode where you're never interacting with that task session? You are indirectly interacting with it by interacting with its parent's session.

> this is somewhere where it might be worth looking at some of the prior art and looking at the human layer or orchestrator amp code I'm not sure what prior might exist in this space

## Observed today (a0548bfe28)

- A Task reaches its Wave only through a Project: `tasks.project_id` →
  `projects.wave_id` (`task_wave_in`, `sqlite/durable.rs:458`). No Project, no Wave.
  This is why a subwave currently needs "a whole project".
- `lf wave create` registers the row, seeds and commits `.lf/wave/<name>/` on
  main, and starts a first Project (`ensure_wave_project`,
  `work/wave/bootstrap.rs:32`). Nesting is by directory:
  `.lf/wave/infrastructure/release/`. `waves.parent_wave_id` exists.
- The repository is a separate kind of scope everywhere: `repository_plans`
  row, `tasks.repository_id`, `PrimaryScope::Repository | Wave | Task`,
  `DefaultScope::Repository | Wave`, `--no-wave`. Settings already resolve
  nearest-first Wave → parent Waves → repository (`scope_in`,
  `sqlite/scope_defaults.rs`). A root `MEMORY.md` already "applies with no
  Wave selected".
- The store work (LOO-934, 937, 940, 954, 928 …) has no Wave: its Tasks sit at
  repository scope and one conversation held the group in its head. The perf
  work (LOO-606, 607 …) sits in `infrastructure` under a completed
  "Performance & Efficiency" Project. Neither group is a thing lf can name.
- `ship-decomposed` splits a branch into Tasks with `lf checkout <child>
  --stack-on <parent>`; nothing records that the children came from one place
  or who is watching them.
- `wave-operate` / `wave-session` already describe one conversation giving
  every started Task a disposition. Session transfer already runs one way:
  a repository or Wave primary can become a Task's conversation.
- The sidebar lists Working Tasks; the Wave view has Backlog / Working /
  Autonomous / Done. A Task is either attended by a person or on Auto-Ship.

## Proposed shape (draft, undecided)

**One tree.** Every repository has a root Wave. Every Wave has a parent except
the root. Every Task belongs to exactly one Wave (`tasks.wave_id`), the root by
default. A Wave *may* have a Project; a Task's Project is the nearest one up
the tree, or none.

**A Wave is a row and a conversation.** Name, parent, one primary Session.
Files (`GOAL.md`, `MEMORY.md`, `config.yaml`), a Project, a schedule and a
chat channel are each optional and added when wanted. A Wave with no files
costs nothing on main. Naming an unknown path creates it:
`lf task create --wave infrastructure/perf …`.

**Growing one out of a Task.** `lf wave create store --from LOO-937`: the
Task's conversation becomes the new Wave's conversation (it holds the whole
picture), LOO-937 becomes the first Task inside it and keeps its checkout and
PR, and follow-ups are filed into the Wave. `ship-decomposed` ends by doing
this.

**Who attends a Task** is one value with three answers: a person (today's
interactive Task), the parent Wave's conversation (new), nobody (Auto-Ship).
With the parent attending, the Task's interactive steps and blocks arrive in
the Wave conversation as steers; it answers with `lf steer` or brings the
question to the person.

**Sidebar.** The Wave tree. Under each Wave, only what is open for you: its
conversation and the Tasks you opened. Everything else is a count on the Wave
row ("4 running · 1 needs you"); opening the Wave shows the board.

**Ending.** Nothing to close. A Wave with no open Tasks and no open
conversation drops out of the sidebar and stays in history; filing a Task into
it brings it back.

### Delete — do not maintain (if the tree lands)

- `PrimaryScope::Repository`, `DefaultScope::Repository`, `repository_plans`,
  `tasks.repository_id`, `--no-wave` as a separate path (staged per schema rules).
- First-Project creation inside `lf wave create`; `waves.promoted_at`.
- The Project-as-membership join in `task_wave_in` and its callers.
- Survives: Wave ids, existing `.lf/wave/**` files, Task identity, checkout
  and PR, tracker bindings, history of retired Waves.

### Shape

Indivisible keystone: root Wave + `tasks.wave_id` + Project climb + file-less
Waves. Additive after it: grow-from-Task, parent-attended Tasks, sidebar open
set, tracker mapping.

## Prior art (web research 2026-11-18, from product docs; wording approximate, not re-checked)

- **Amp**: flat threads linked by handoff, mention or fork; the thread map
  names two shapes, hub-and-spokes and chain. Handoff drafts the new thread's
  opening prompt and file list for the person to edit. Archiving a parent
  leaves children, which move to the top level.
  (ampcode.com/news/handoff, /news/thread-map, /docs/markdown/threads)
- **HumanLayer / CodeLayer**: Project → Task → Session; `/hl:orchestrate`
  launches tasks in separate worktrees and moves artifacts between them. No
  parent/child link found in the UI. One bell lists sessions waiting on you.
  (docs.humanlayer.com release notes 0.161.0, 0.181.0, 0.182.0)
- **Devin managed Devins**: coordinator session with child sessions; "the
  coordinator is woken automatically, even if it is asleep, so it doesn't
  poll"; each child has its own link and can be messaged directly.
  (docs.devin.ai/work-with-devin/advanced-capabilities)
- **Claude Code agent teams**: lead → teammates, no nesting, lead fixed. A
  teammate's permission prompts appear in the lead session. Idle rows collapse
  to "N idle agents". Listed failures: teammates lost on resume, status lag,
  the lead doing the work itself, same-file overwrites.
  (code.claude.com/docs/en/agent-teams)
- **Codex subagents**: approvals surface from inactive threads with a source
  label and one key to open that thread.
- **Factory Missions**: "the orchestrator is an agent, and you can talk to
  it"; converting a normal session into a mission later "loses the planning
  context"; fresh validators raised 34% of implementation work as fixes.
  (docs.factory.com/cli/features/missions)
- **Vibe Kanban**: "Create Subtask" pre-fills the parent's branch as base.
- **Linear**: rejected sub-issues for agent delegation; sub-issues are
  "intentionally independent, each with its own status and owner".
  (linear.app/now/our-approach-to-building-the-agent-interaction-sdk)

What it changes in the draft:

- The server wakes the Wave conversation on a child Task's question, block or
  finish; the conversation never polls. (Devin, Claude teams)
- A child's question arrives in the parent labelled with its Task and one
  action to open that Task. (Codex)
- Growing a Wave from a Task must carry the planning context: the reason the
  Task's conversation moves up rather than a fresh one starting. (Factory)
- A child Task's brief is a draft the person can edit before it runs. (Amp)
- Siblings stack on the originating Task's branch by default when they
  depend on it. (Vibe Kanban; `lf sync <ID>` already does the stacking)
- A child's "done" is not taken on its word: the Graph's own review and CI
  already stand between a Task and merge; the Wave conversation reads those,
  not the child's summary. (Factory)
- Nothing found nests more than one level with lasting identity. Groups that
  live only in a session die with it (Claude teams). The store row is the
  difference here.

## Answers (Jack Heart, 2026-11-18) to questions 1–6

> 1 -- the task is closed and the session becomes the session for a wave, or the task "becomes" a wave. maybe the data model is more fluid between wave/task
> 2 - hmm. i think if its shared it needs to be a task [possibly with subtasks?]. otherwise its just a private folder of tasks basically
> 3 - c probably
> 4 - literally everything. the main workflow is you just talk to the wave session and ithandles all hte tasks for you, though of course sometimes you will jump into the details, and it mostly handles when to launch them
> 5 - yees
> 6 - a

(1 = where the Task's conversation goes on growth; 2 = shared or local
file-less Wave; 3c = Linear parent issue with sub-issues; 4 = what the Wave
conversation may do for its Tasks; 5 = root Wave gets files; 6a = one word,
"Wave".)

## Draft after the answers (proposed, undecided)

Answers 1, 2 and 3 point at one mechanism: **a shared Wave is a tracker issue
with sub-issues.** A Wave row can carry any of three things, each optional:

| Carries | What it is | Others see it |
|---|---|---|
| nothing | a private folder of Tasks on this Machine | its Tasks only, ungrouped |
| an issue | parent issue; its Tasks are sub-issues | yes, in the tracker |
| `.lf/wave/<path>/` | goal, memory, config, schedule, chat; optional Project | yes, in Git |

A Task that grows becomes the issue-carrying kind: LOO-937 is the parent
issue, the conversation is the Wave's, follow-ups are sub-issues.

**The Wave conversation runs its Tasks**: files, briefs, launches, steers,
answers their questions, lands. The server wakes it when a Task asks, blocks
or finishes. **Open is the one switch**: a Task you opened is yours and its
questions come to you; close it and it goes back to the Wave. The sidebar
shows what is open; attendance and visibility are the same fact.

Root Wave: `.lf/wave/GOAL.md`, root `MEMORY.md`, `.lf/config.yaml` as its
config.

Undecided: what happens to a branch with code when its Task grows — see
questions.md 7. That answer picks between two tables with a conversion and
one node type.

## Explore round 1 (Jack Heart, 2026-11-18)

Worlds shown: 1 the outline (sidebar is the Wave tree, open set only),
2 the conversation is the Wave (no group to make; Tasks in its header),
3 the inbox (one list of passed-up questions across Waves), 4 zoom (work is
one kind of thing at every size, a conversation at each level).

> lets /explore some UXes before worrying about getting the data model perfect

> i like 1, 3, 4. dont like 2. 1 and 4 are most important, 3 is something we can add on later

Round 2 of `scratch/explore.html` joins 1 and 4. Nothing is chosen to build yet.

## Explore round 2 (Jack Heart, 2026-11-18)

Questions: 1 what was wrong with "the conversation is the Wave" (b = Tasks
living only in a conversation's header); 2 conversations for a thing with a
branch and children (a = one that also writes its code, c = one that stops
writing code); 3 who decides to split; 4 what the outline lists with nothing
opened; 5 what LOO-937 is called once it holds Tasks; 6 when everything
inside has landed (a = leaves by itself, b = stays until closed).

> 1b 2a/c: a wave can still publish PRs [waves and tasks both get unique worktrees]. 3 any of the above possible. most likely: there is a cLI for doing it, you mostly tell the session to do it, it may do it of its own accord 4 not sure. i dont think "what asks" is the right way to think about things, its more about where things are in their task graphs. but start with showing more. 5) tempted to say that its both a wave and a task. task: concrete shippable outcome that results in ...exactly 1? PR [i have struggled with this where in practice often nice to be able to continue in one conversation; maybe this is enough to say that when that happens basically a task "graduates" from a task to a wave[.  Maybe all tasks are waves, or else we just say a wave is work that isnt a task
>
> 6 a or b. it doesnt leave just automatically because everything closed, but it will usually close itself and/or you can close it

## Explore round 3 (Jack Heart, 2026-11-18)

Outlines shown: A tracks (each row draws its whole Graph), B stops (inside a
Wave, work grouped by where it stands relative to the person's interactive
steps), C lanes (one Wave as a pipeline with phases as columns). Questions 2–7
of that round were not answered.

> B. i dont think C works because theyre not guarneatteed to be the same

Consequence drawn here, not said by Jack: anything that names phases or steps
shared across Graphs fails the same way. Round 3's B had a "landing" group
keyed on the `code` Graph's step names; round 4 keys groups only on what every
Graph declares (`interactive:` steps) and on started / not started.

## Explore round 4 reply (Jack Heart, 2026-11-18)

> 2: just the current step, with coloring [and maybe more] based on whether its a stop for you or not
> 3 c + b. stacking should likely force a L style representation but you can rearrange siblings

Read here as answers to round 3's questions 2 (how fine is "where it stands")
and 3 (order inside a Wave: c = by what stacks on what, b = the dragged
order); round 4's question 3 had no c. Not confirmed.

Tension noted, not resolved: stack order plus dragged siblings is a tree
order, and round 4's group headers (at your stop / heading to you / past) are
a different order over the same rows. Round 5 draws the tree with the step
coloured and no headers.

> i think the labels should be icons so its less noisy
> at your stop / heading to you etc

(Jack Heart, 2026-11-18, while round 5 was being drawn.) Round 5 now puts one
icon per row for the four standings and icon counts on Wave rows; no headers.

## Round 5 reply (Jack Heart, 2026-11-18)

Numbers are round 3's: 4 wording for Task and Wave, 5 what graduates a Task,
6 what LOO-937 is called afterward, 7 whether a Wave's own PR runs a Graph.

> 4) not sure, what do you think
> 5) probably when you launch a task, you can file it with a parent, and if the parent is another task, it becomes a wave. need to think about whether this implies stacking or how it interrelates with stacking
> 6) this makes me think that we maybe want to maintain 937 as the task and rebind its session to a new wave; 937 itself gets a forked session maybe
> 7) not sure, might be simpler to not have PRs associated with wave convos. had issues in the past with wave sessions just doing work in their own worktree or branch. have not been using the wave sessions that much in practice so would rather redesign around whatever is right for these more emergent waves

This revises the round 2 reply ("a wave can still publish PRs [waves and
tasks both get unique worktrees]").

## Where the draft stands (proposed, nothing chosen to build)

- **Two kinds, complementary.** A Task ships: one outcome, one worktree, one
  PR, a Graph. A Wave runs: a conversation over Tasks and Waves; it has no
  branch and publishes nothing. Zoom survives as navigation (repository ›
  infrastructure › store › LOO-937), not as one merged type.
- **Filing with a parent.** `lf task create --parent LOO-937 …`: when the
  parent is a Wave the Task goes inside it; when the parent is a Task, a Wave
  is made holding both, the Task's conversation is rebound to that Wave, and
  the Task gets a forked conversation. LOO-937 stays LOO-937 with its
  worktree, PR and Graph position.
- **Parent and stack are two facts.** Parent says who runs it. Stack says
  which branch it starts from (`--stack-on`, `lf sync <ID>`). The outline
  indents for parent and draws the L for stack. Default when filing under a
  Task: open.
- **"Continue in one conversation"** is the conversation moving up: the next
  PR is a new Task it files.
- **Outline**: tree in the person's order; stack as L; one icon per row for
  its standing relative to the person's interactive steps; step named in its
  own Graph's words; icon counts on Wave rows.
- Carried from before: root Wave per repository with files; Project found by
  climbing; a shared Wave is a tracker parent issue with sub-issues, a private
  one is a local folder; a Wave closes when its conversation or the person
  closes it, never just because its Tasks finished.

## On chapters and KRs (Jack Heart, 2026-11-18)

> i will also note taht i have not been using the chapter system, KRs, etc. Can definitely believe in a world where that does get used more by bigger teams, dont think its a terrible idea, but it doesnt really fit me as a solo dev who still likes to be relatively hands on in the code design. possible we need to invest more there, but also willing to throw out what we have for that stuff for now and then bring it back in at a later date

Measured at a0548bfe28 (grep over rust/, swift/, python/; counts include tests):
KRs 194 lines in 49 files; metric targets, instruments and observations 242
in 50; Project activation and readiness 91 in 20; `current_project` /
`selected_project` 58 in 23. Store: `projects.krs`, `.metric_targets`,
`.iteration`, `.status`, `.legacy_current`; `project_transitions`;
`metric_instruments`; `metric_observations`; `waves.current_project_id` and
seven `project_activation_*` columns. Commands: `lf wave update-project`,
`lf wave edit-project`, `lf wave plan bind`. The "current Project" is the join
that makes a Task need a Project to be in a Wave.

Proposed cut (undecided): a Project becomes only an optional link from a Wave
to a tracker Project. No current Project, no chapters, no activation, no
transitions, no KRs, no metric targets. Schema removal is staged per the
migration rules.

## Growing in place (Jack Heart, 2026-11-18)

> A Wave runs isnt really a semantic match. better verbs?

Offered: carries, steers, tends, conducts, "keeps Tasks moving". No pick yet.

> can we move the session to a different worktree also? Im not sure exactly the underlying mechancis, but the point being that the user experience when a task grows into a wave, it should be as seemless as possible if the user is just sitting in that chat session

Observed (docs/dev/reference-models.md, "A Session's permanent assignment"):
- Binding a Session to a Task already "immediately sets its cwd to the Task's
  worktree", committed atomically with assignment and primary choice,
  "preserving live input and attachment"; "already-running children may keep
  their original directory".
- The same paragraph forbids the direction growth needs: assignment is
  "write-once: null to a Task … Reassignment and clearing are unavailable".
- Code exists for a moved Session reopening at a new directory
  (`reopen_moved_session`, `sqlite/sessions.rs:1826`) and for copying a
  transcript between provider homes (`transcript::copy_between`). Not read in
  depth. Not verified: whether Claude and Codex resume the same thread from a
  different directory.

Proposed: growth releases the Task's conversation to the new Wave and gives
it the Wave's directory, which is not a checkout; the Task's own conversation
is admitted lazily as a fork when someone opens it. The write-once rule is the
precondition to drop.

> you should look at the outstanding tracker redesign task to see how that fits
> 1) look at tracker redesign task 2)a 3)a

(Jack Heart, 2026-11-18. Numbers are the chapters questions: 1 how deep the
cut goes; 2a live metrics go too; 3a the cut is its own Task landing first.)

> what do you know this task is itself a wave

(Jack Heart, 2026-11-18.)

## Fit with the plan and tracker redesign (LOO-938 moving, LOO-980 idle)

Read from `loopflow.loo-938-send-every-planning/scratch/plan-design.md`,
`plan.md`, `questions.md`, `explore.md` on 2026-11-18; that design is "not yet
chosen" in its own notes and its Graph is mid-implement.

What it says that bears on this:
- Plan records travel as stamped fields in per-Machine Git refs
  (`waves/<id>.json`, `projects/<id>.json`, Tasks), and through a hub. A
  repository's people meet through Git or through a tracker, never both.
- "A Wave links to a Linear Project." "Wave membership comes from the link. A
  Task's Wave is the Wave linked to its Linear Project, so everyone derives
  the same answer and nothing is stored." "Nothing Loopflow-only is stored on
  a tracker item."
- Jack Heart there: "i think we also should just need a default initaitive or
  team for creating NEW waves, but we shouldnt need to care about the
  team/initative in any other way".
- Two orders: backlog order is the Project's and shared; "your order of your
  claimed tasks is NOT the proejct order".
- Run state is not shared between people: "I think mostly or exclusively no."
- Its records still carry a Wave's parent and "selected Project".

How the subwave draft sits on it:
- **Fits.** A file-less Wave is one more plan record. On a Git repository it
  reaches a person's other Machines and teammates with no extra step, so
  "private" there only means "no files on main".
- **Fits.** Outline order is the private claimed order; Wave order joins it.
- **Fits, and decides the depth of the chapter cut.** "A Wave links to a
  tracker Project" is all a Project needs to be. Removing Projects entirely
  would contradict "your slice of a Project".
- **Collides.** A Wave with no Project puts its Tasks in an ancestor's Linear
  Project, so "membership from the link" names the ancestor. A parent issue
  repairs it without storing anything Loopflow-only: a Task's Wave is the Wave
  whose issue is its parent, else the Wave linked to its Project.
- **Open.** A Wave's conversation is run state, so on a shared Wave each
  person would have their own.
- **Sequence.** LOO-938 is rewriting how Wave and Project records are saved
  and still carries `current_project`. The chapter cut touches the same code.

## The verbs (Jack Heart, 2026-11-18)

> I think you are conducting the wave. The wave carries indeed, and it brings the tasks with it. thats kinda what im going for

Wording now used in the draft: a Task ships. A Wave carries, and brings its
Tasks with it. You conduct the Wave.

## Feedback given, and dependencies (Jack Heart, 2026-11-18)

> give me your honest feedback on this design

Given in conversation: the conductor conversation is the untested bet; who
answers at a stop is the crux; graduation exists only because conversations
start inside Tasks; a file-less Wave's only memory is its conversation; open
merges looking with taking; the root Wave, tracker sharing and depth beyond
one level are ahead of the evidence. Suggested first build: direct Wave
membership with instant file-less Waves, the server waking the Wave's
conversation on its Tasks' events, a week of real use.

> yeah another task here is moving from raw stacking to dependency management
> in particular, that would basically allow you to say "start this task whenever the other one lands"

Observed: stacking is one parent per Task (`task_checkouts.parent_pr_id`,
`lf task run LOO-13 --stack-on LOO-12`, `lf sync LOO-12`), with a cycle check
in `sqlite/children.rs:885`. Nothing records "after" without a shared branch,
and nothing starts a Task when another lands.

Proposed (undecided): one relation, "LOO-954 is after LOO-937".
- By default the later Task waits and starts from main when the earlier one
  merges. The server starts it; no conversation has to be awake.
- Stacking is the same relation with "start now, on its branch", for at most
  one of a Task's dependencies.
- A Task may be after several Tasks; a branch can only stack on one.
- The outline's L is the dependency, not only the stack. "Waiting on
  LOO-937" is a fifth standing.
- Works on plain Tasks with no Wave: `lf task create --after LOO-937 '…'`.
- Linear has blocks / blocked-by relations between issues, so the relation
  can travel without a Loopflow-only field (API shape not checked).

## The pattern that is in use (Jack Heart, 2026-11-18)

Replying to the feedback that the conductor conversation is unproven:

> Right but I HAVE been using task sessions to manage its followup tasks. Im trying to make that interaction pattern natural for the product rather than fighting against the task lifecycle

Corrects the feedback: the proven pattern is a Task's conversation managing
the Tasks that follow from it. Growth out of a Task is therefore the main
case, not machinery to design away; "start the conversation at the Wave" does
not match how the work begins.

Where the Task lifecycle plausibly fights it (from docs and schema; which of
these bite in practice is not yet confirmed):
- The conversation is bound to the Task for good (write-once assignment).
- The Task finishes at its merged PR while its conversation still has
  follow-ups in flight; the Task leaves Working and takes the conversation
  out of view.
- `task_retirements` removes the Task's checkout and branch after completion,
  which is the conversation's directory.
- Follow-ups filed from it record no link back, and nothing wakes it when one
  stops or lands.
- Its interactive steps (`human-review`, `verify`) arrive in the same
  conversation that is now busy with other Tasks.

## Investment and debt (Jack Heart, 2026-11-18)

Replying to the feedback that "literally everything" and "hands on in the code
design" contradict each other:

> well,  there's kind of this cycle of investment and debt where the more you invest in designing a task, the more you can hand it off to an agent to run without having to verify it (because you've already pre-specified the requirements)  what I find is that if you stay totally hands-off in the code and you're just acting at the Wave level (which is big high-level things for a project), then you're accumulating a lot of comprehensive debt   but if you are investing, for each task, really in designing that task and reviewing that task, then for the follow-ups to that task, you don't really need to be as involved in checking those (because they're mostly already pre-designed and pre-spect)

Drawn from it (proposed, undecided):
- The stop question is answered per Task by how it was designed, not by one
  rule for the Wave. The Task the person designed and reviewed keeps its
  stops. Follow-ups filed from that conversation carry an excerpt of that
  reviewed design, and the conversation answers their stops against it.
- What comes up to the person is a follow-up that needs something the design
  did not settle.
- The Wave's memory is the reviewed design it grew from, not only its
  conversation. `ship-decomposed` already hands each child a design excerpt
  (`lf checkout <child> --stack-on <parent> --design <path>`).
- A follow-up with no design behind it ("oh, also fix this") is new
  investment and keeps its stops unless the person says otherwise.

## Replies to feedback points 3, 4, 6, 7, 8 (Jack Heart, 2026-11-18)

On 3 (graduation is a lot of machinery; start the conversation at the Wave):

> Things are made complicated by the fact that we don't want to assume that a project literally starts with Loopflow and/or that everyone in a project is using Loopflow. We do have to accommodate the idea that there may be a lot of outside context and layers of project management that we don't have access to. We can't really assume that the entire system lives in Loopflow.   But if you are a project that starts in Loopflow, then it just is the case that as you work on the project, you're going to start doing things. You're going to find that those things need follow-ups or that there's a lot of work in a single area that you want to do. You're going to want to group your work together and have the sort of graduate.
>
> There is going to be some sort of process where every time task work gets defined, there's an opportunity for that work to be related to past work or future work (in a way that makes it easier to understand and manage).

On 4 (a file-less Wave has no memory but its conversation):

> I'm not sure what you mean by fileless wave. If you mean this new breed of wave that's emergent and not a long-standing wave in the codebase, first of all they can still maybe spin up their own .dotio/wave and then delete it when they're done. I think that is something we could consider   it also still has access to the entire repo. If you assume that it's no worse off than a previous task, which always had its parent waves repo, and we're imagining that these emergent waves will have parent waves (if nothing else, the repo itself)

(".dotio/wave" read here as `.lf/wave`.)

On 6 (the repository as a real root Wave):

> Yeah I think this is something where we shouldn't just do it. There's tension on both. There are reasons not to do it. I haven't done it for reasons. This product may push it over the edge with its reasons to do it but I don't think this needs to be a goal in itself. It's just an option on the table in order to unify the design further

On 7 (shared Waves as parent issues):

> Again I think this is something where there are two different things:
> 1. An individual whose work is managing and who's doing a lot of concurrent work with agents is going to want to find ways to group that work into folders.  And we want to work well with that.  And if you want to share your work with others, then we should make it easy to share your work, including your organization scheme.  Likewise if you are working within the context of a larger company that has some sort of project organization scheme, we should make it easy to make that line up with what you're doing

On 8 (depth is assumed, not observed):

> I think it could be totally fine to say that we limit depth to a certain number, maybe even 2. You could have the repo and then some sort of project wave and then within the project you get to have one more level. That could be the place to start

## Replies to the second round of feedback (2026-11-18)

Jack Heart, on "the Wave is doing work the Task relation could do":

> Yeah I agree that dependency tracking doesn't need waves and it's its own feature. We could design it separately. I think that's correct. I do have some concerns that once you have too many different ways of relating tasks to each other, it becomes hard to manage or understand

On moving the conversation splitting the context:

> Right. I think we should design the product in a way where it doesn't feel like the context gets split because the conversation that the human is actually using stays the same.
>
> In a world where you grow tasks, you don't need to jump into the subtasks in the same way that, when a task session or when an LM session launches subagents, by default the human doesn't need to jump into the subagents.

On "After is two things under one name":

> All right you've introduced your own terminology, like "after" and "grow." Can we talk in plainer terms first? Maybe let's talk more about what you like and not like about the product design that I've expressed (in terms of the things I'm saying in my goals and the problems I'm trying to solve), rather than critiquing your solution

On `--parent` doing three things:

> Yeah I think we probably need to have a clear model of this. Fewer flags are probably better though. As I was saying, I think once you have too many different ways of expressing similar relationships, that's probably not the right model

On a conversation answering stops, and Auto-Ship:

> Right. This is another example of how there might be slightly too many ways of expressing the same thing already. You can ship with `-b`, you can run the auto graph, and then there's the `ship` function. I kind of want to delete the `ship` function or `ship` CLI or at least make `ship` just a shorthand for one of the other two things rather than it being sort of like its own state

Point 6 (the reviewed design has no home) was quoted with no reply under it.

Also asked for: "prototypes of the actual full app… with some sequences, different data states". `scratch/explore.html` round six was written for that; it was not looked at rendered.

Jack Heart, on where a shared plan lives:

> Right. One thing that I've noticed is that when you do something like this (and another example of this, by the way), you can look in manabot for the standard, for supporting all the cards in standard.
>
> That's something where I started a task. It was clear that it was actually going to be a lot of tasks. I basically had to check in a doc (which is like, "Here's the plan for this project") and then close the task.
>
> I do think there needs to be some sort of clear answer about when you want to have a shared plan for some subset of tasks. Where does that live? Is it in doc/? Is it in .lf/? Is it somewhere else?

Observed in manabot (2026-11-18): the Standard plan was published by PR #425
(2026-11-04) into `docs/research/standard-2026-11/` — README, `milestone.md`,
audits and a ledger per set. It now sits under
`.lf/wave/coverage/research/standard-2026-11/`. The Wave's `GOAL.md` gained an
"Accepted scope update" paragraph linking to it, and the plan lists its Tasks
by ID (MTG-241 to MTG-245) in prose.

## One type? (2026-11-18)

Jack Heart, on "Wave is being asked to mean two things":

> Yeah it may be that rather than making subways, we're making subtasks. I think I used to work at Asana. The head of product and co-founder of Asana used to say that his biggest product regret was having projects and tasks be separate and not having it just be subtasks all the way down   I'm kind of inclined in that direction. I do kind of want to fuse Wave and task into one type. The issue becomes: there are issues with it. It's weird if you have a completed Wave with incomplete tasks.
>
> I will say that it's not uncommon with Asana, where you have a task that's complete and some of the subtasks don't get completed. In that case what that usually means is that the subtasks basically count as completed, at least in some sense. There's some combination of completed and discarded

On "a task becomes a wave" versus "the conversation stays the same":

> Right. I think this points again towards more of a tap. Every wave is already a task and the model might be every wave, sort of everything. You might need to move towards a wave is many tasks. When a wave is many tasks, though, then that's sort of like the wave itself becomes confused with its children.
>
> You could have a task as many PRs. That is, I think, kind of the state today. Also I feel like it becomes a little bit confusing when the task is complete and it does just introduce more levels of hierarchy that are not usually needed.
>
> I do kind of think maybe the right thing is everything is a wave and/or maybe wave gets called work: everything is work. You can use the sort of task fields or you can use the subtask. You can have subtasks and you can sort of do whatever you want with it. Maybe the closest notion here is a Notion page.
>
> I do think that maybe just in general Notion is the right thing or is a good thing to look at here, where Notion does have tasks. I will say Notion's handling of tasks is pretty weak. I multiple times have looked at using Notion as an underlying task tracker and have decided that its API just isn't really designed that way. It really wants its tasks to be super contained within, very hierarchically managed, rather than sort of their own first-class objects.  But I do think that Notion kind of gets this idea: everything is the same, everything can have a unique type, and there's kind of arbitrary hierarchy. You might want to study their data model

On who decides when Tasks launch:

> Yeah I don't know. I guess I would think about it like this: this is somewhere where people management becomes a kind of metaphor. It takes judgment. Sometimes mostly, we already have a split where each task has a worker and a conversation. The conversation is the way for the human to drive it but tasks can also be steered.
>
> Now the parent is another way to drive the task. It's slightly less direct and it's a slightly different context. The way that the wave might manage when to launch them is by setting the right dependencies between the tasks.

On the two-level limit:

> I agree. I would probably prefer not to have two levels. Maybe five levels or something is right. I think at Asana the subtask levels are something like that. It's the kind of thing where it really tends to be a pretty bad idea to want to nest super super deep and generally flatter hierarchies are better. That's part of why I don't. I'm open to having pretty low limits but also if we don't need a limit, we should avoid it

Notion's data model, from its engineering post (notion.com/blog/data-model-behind-notion, read through a summarising fetch 2026-11-18): every block has an id, a type, properties, `content` (an ordered list of child ids) and `parent`. "Changing the type of a block doesn't change the block's properties or content"; a property the new type doesn't use is kept and not shown. `content` gives order and rendering; `parent` is followed upward for permissions. Indenting moves a block into the preceding block's children. Notion sub-items are a parent-item/sub-item property pair on one database; dependencies are a separate feature; deleting an item deletes its sub-items. Asana's subtask article did not load; five levels and "subtasks do not inherit the parent's project" are from memory.

Jack Heart, on docs beside Tasks:

> Yeah it's possible that we should expand from just having tasks to having docs. You can have your task tracker and you can have your docs storage. Your planning docs can be in Google Docs or Notion and your task tracking could be in Linear and Asana. Asana and Loopflow know how to just navigate between both of them

Jack Heart, on Asana subtasks not inheriting the parent's project:

> Yeah I think in practice this was the right call for Asana and we definitely looked at both ways. I don't think that that necessarily holds or turns over here but I will say that some tasks do inherit their projects from a data model perspective. If you run a search query, they do count as being in the project

Jack Heart, on agents writing plans into an outside doc tool:

> Well yeah. I think one thing that's nice, though, is that these Google Docs and Notion have good APIs. It's not like they're unaware that agents run the world now so I think this is totally solvable.
>
> In the same way that people teams do tracking in Linear, therefore if you want plans to be exposed and editable by humans (for example, humans in a meeting), I think that logic kind of holds up for docs as well. This is kind of the first I'm thinking of: having a special docs storage but I think it's a good idea.

Jack Heart, on a built-in Git store for docs:

> I guess, question: with the task tracking we offer Git as a store. In case you don't have something else set up, we should do the same here. For task tracking we chose to put it in a separate repo. That way it kind of makes it live. I think we should probably do the same, or not a separate repo but a separate... I don't even know the Git word. I think we should probably do the same for the built-in Git-based docs tracker. What do you think?

Jack Heart, replying to "Which goes first? a. Work with children, since the rest lean on it" and "Should comments and edits in a doc reach the owning conversation? a. Yes, as a steer" (my reading of which questions "1(a) 2(a)" answers):

> 1(a) 2(a) -- this is tricky though and robably wants to be its own task. in theory iwould like it to reach all the children, so all the task in wave when a wave doc is edited. However, i think we need to batch changes together, since doc edits are much more of a stream than task tracking edits

Jack Heart, on two people editing the same doc:

> Yeah do we need our own CRDTs here? t

Jack Heart, 2026-11-18, after the plan was rewritten as "Work with children":

> ok, so lets start to beta-test this overall workflow now.
>
> - Start work on builing out the plan for work with children.
> - File the other tasks. Start which ever ones arent blocked on work with children, if any

And, on batching doc edits (my reading: "1 2", what closes a batch and who gets it):

> 12 - we can start with c and b i think,

Jack Heart, on closing work with open children (3) and children in the sidebar (4):

> 3: i think probably close by default
> 4: I think this is something we'll need to play with

Jack Heart, on what Work can be (after five Tasks were filed and two started):

> i guess in practice i am proposing sort of three possibilities for WOrk
>
> - It's a project: persistent, relatively long-lived, shows as a group in the tracker [which only really matter for linear and shared teams]
> - its a task: transient, compeltable
> - its a folder: conceptually like a relatively short lived project. could be one with the cost that it could create noise in your group's tracker. not sure how big a deal that is. could also just be more like a "local project" where other people see the group of tasks in the parent's project, but you know its a related group. Or could be a task with subtasks.

Filed 2026-11-18 (short ids until Linear syncs): c6a874 work with children (this checkout and conversation, running), 43bf9b chapter removal (running, stacked on LOO-938), 3de47e start a Task when another lands, 2f289b doc store, be1e9f plan edits reach children. "One way to ship" is the existing LOO-968.

Jack Heart, on a short-lived group being a real tracker Project:

> I dont know about linear but at least in asana you can archive / complete projects. again im not sure exactly what costs tehre are to having a lot of these. IMO conceptually this is the most right but im not sure how it works in practice at the task tacker companies

Linear, from its docs through a summarising fetch (2026-11-18): a project needs only a name and a status. "Issues can only be associated with one project at a time." Closed projects follow the team's auto-archive period and can be restored; deleted ones are recoverable for 30 days. No project-inside-project was found; initiatives group projects; projects have milestones. Sub-issues "inherit the parent issue's team, priority, and project"; two optional team settings: "When all sub-issues are marked as done, the parent issue will also be marked as done automatically" and "When the parent issue is marked as done, all remaining sub-issues will also be marked as done." Asana tasks can sit in several projects at once (from memory, not fetched). Observed here: `.lf/wave/infrastructure/GOAL.md` and `product/GOAL.md` carry a `linear: initiative:` id; LOO-938's design says "A Wave links to a Linear Project".

Jack Heart, on a parent issue being the tracker's own structure:

> Probably the best default plan, though we would have to be aware of how much nesting is allowed in linear/asana

Jack Heart, on Wave versus Initiative in Linear:

> i think wave = initiative was an old design. now repo = initiative, but i will note i dont think that relaly scales to larger deployments

Jack Heart, on where the tracker mapping came from:

> initially i was designing from the perspective of "let me make a playgorund thats safe for me to experiment in and doesnt mess with other peoples stuff"

> i think now the model is more "how can we work well with people's existing stuff"

> i am pro sharing on new goroups

> one other thing we havent discussed: a wave is also a container for expressing loopflow settings

Jack Heart, on settings (a Wave "is also a container for expressing loopflow settings"):

> that is definitely something that should apply to these new intermediate groups

> lets focus on workign best with linear. waht design would that point to. In practice, how do large companies manage their project layouts?

Linear's layout, from its docs and its Method page through a summarising fetch (2026-11-18): "Teams are the primary organizational unit"; "An issue belongs to a team"; sub-teams nest one level on Business, "up to 5 levels" on Enterprise, and may inherit statuses, cycles, labels. "Initiatives sit above projects and represent broader strategic efforts", have an owner, a lead team, a target date, sub-initiatives, documents and updates. Milestones "further organize issues inside an individual project", are flat, and "If a milestone grows large, you can convert it to a project." Method: "Every project should have a named owner who's responsible for writing the project brief and delivery"; specs short; "2-week cycles are the most common". Linear has no repository object. No company case studies were read; "how large companies lay it out" is inferred from these pages.

Jack Heart, on project layout in Linear:

> my guess is that you generally awnt to have projects scoped to relatively sized teams, ratehr than giant top level "infrastructure" and "product" style proejcts
>
> and if therre is no project-nesting, then you are required to do that

## Kickoff probes and readings (2026-11-18, headless, nobody present)

Resume from another directory, run in throwaway `/tmp` directories with the
harness's `CLAUDE*` and `LF_*` variables unset:

- Claude Code 2.1.328: `claude -p 'Remember the word marmalade…'` in `a/`;
  `claude -p --resume <id>` from `b/` answered "marmalade"; after `rm -rf a`,
  the same from `c/` answered "marmalade" and its Bash `pwd` printed `…/c`.
  The transcript stayed at `~/.claude/projects/<encoded a>/<id>.jsonl` and
  grew there (44 then 68 lines). Interactive `--resume` not run.
- codex-cli 0.170.0: `codex exec --json` in `xa/` gave a `thread_id`; after
  `rm -rf xa`, `codex exec resume <thread_id>` from `xb/` answered "quince"
  and `pwd` returned `…/xb`. App-server `thread/resume` not run.

Linear's published schema (github.com/linear/linear,
`packages/sdk/src/schema.graphql`, fetched 2026-11-18): `IssueCreateInput.parentId`
"Setting this makes the new issue a sub-issue; the parent can belong to
another team if you can access it"; `projectId` "If omitted for a sub-issue,
the parent's project is used when it includes that team";
`IssueUpdateInput.parentId` "Pass null to make this a top-level issue… An
issue cannot be its own ancestor"; `Issue.parent`, `Issue.children`. Doppler
holds only Linear and Asana OAuth client secrets, so no live call was made.

Code readings that moved the plan (a0548bfe28):

- Merge never completes a Task; `tasks.planning_completed=1` does
  (`sqlite/durable.rs:346`). Retirement follows completion or abandonment,
  15 minutes later, by a once-a-minute `lf task retire` (`task_retirement.rs:219`).
- Retirement writes nothing to `agent_sessions`; Sessions keep a dead `cwd`.
- The Machine server already wakes idle conversations with Task updates
  (`server/wake.rs:46`), only for the Session's own Task, and a merge is not
  an observed fact (`harness/task_updates.rs:56`).
- Several PRs in a row on one Task are live (`ops/task/delivery.rs:120`,
  `tests/task_pr_range_tests.rs:1038`); `after_merge` and `next_slug` are dead.
- No Task-to-Task parent exists anywhere; `waves.parent_wave_id` /
  `WorkValue::WaveParent` is the working pattern.

> if so, then it seems like we might invert the design somewhat and basically allow hierarchy to be grouping ontop of projecst

> what do they use sub issues for

Linear's sub-issues page (summarising fetch, 2026-11-18): "Use sub-issues to break down larger "parent" issues into smaller pieces of work"; create them "when a set of work is too large to be a single issue but too small to be a project"; "also ideal for splitting up work shared across teammates"; "Sometimes an issue grows so large it's more appropriate to turn it into a project instead." Converting a parent adds the original issue and its sub-issues to the project as standalone issues and removes the sub-issue links.

Jack Heart:

> so lets take a look at some of the examples from today:
>
> - store rework
> - perf work
> - standard card file
>
> which of these map well to issue -> subissues? which to new projects?

Jack Heart, on which examples map to sub-issues and which to Projects:

> i would say all could be projects, and only the store really maps to subissues. the rest are too big to be subissues

> can you look more into how people actually use teams, initiatives, projects in practice? i ave a hard time believign all projects end in reality

## Concept review readings (2026-11-18, headless, nobody present)

- `session_tasks()` (`sqlite/task_work.rs:66`) is "the Tasks a Session belongs
  to" for steer audience (`steers.rs:411`), `conversation_task`
  (`moves.rs:165`), the context block, the steer marker, session completion,
  the orphan filter and retirement. Several match exactly one Task.
- A Task update is written at every step start: `RunObservation.step` is part
  of the compared reading (`task_updates.rs:157`).
- Retirement waits while the Task's terminal is viewed (`ViewerHold::Viewed`,
  `task_retirement.rs:226`).
- `gh pr view`: #1971 is closed; LOO-938's open PR is #1983 on the same branch.
- `lf task reopen` exists (`lf/mod.rs:2233`); `TaskDetail` already carries
  `stack_parent`; `TaskRow` is the row lists use.
- This file gained lines from the Task's live conversation while the review
  ran (sub-issues versus Projects for the store, perf and Standard work).

How Linear projects are used in practice (web search and summarising fetches, 2026-11-18; thin evidence, few named companies):
- Linear changelog 2024-03-19: a Maintenance project status "to capture evergreen projects as well as projects that have shipped but are still actively being worked on". Linear's projects FAQ: "Technically yes, you can create a project without a completion date".
- Linear, large-company guide: "a small number of top-level teams"; a team when there is "A durable group of people who regularly work together"; projects "typically run for weeks to months"; a project, initiative, label or view for "A temporary body of work".
- Linear on itself: a project is shared work taking "over two weeks"; engineers lead, specs 1–2 pages, weekly updates; only mobile and infrastructure are specialised teams (CEO, via Lenny's Newsletter, as summarised).
- Langfuse handbook: "Projects need project management, so only open one for real work"; "Nothing rolls forward silently"; initiatives "Rare"; product-area labels for filters.
- r/Linear (two threads, replies only): a core feature "with no definitive end date"; "We utilize labels to identify long-term projects."
- hilton.org.uk: "Like Jira projects, Linear teams have members and no end date."

> i agree with linear that best practices are to have proejcts end; this is basically the chapters idea

## 2026-11-20

Jack Heart: "how are we doing"
Jack Heart: "what exaclty have we ecied to do"

Observed: PR #1986 open for LOO-996 at human-review, base is LOO-938's branch, CI suites skipped (17 checks: 2 success, 15 skipped). LOO-995 stopped at sync: "stack parent jack/loo-938-send-every-planning-change-through-one-queue closed without merging" — LOO-938's PR #1971 is closed and #1983 is open on the same branch.
Jack Heart: "a" (to: LOO-995 stop — a. relaunch as is and see whether sync finds #1983)

Jack Heart: "ok lets lf sync and group. What has landed now and what is left in this iff"
Jack Heart, on "1. Subtasks, not subwaves. A Task can have child Tasks, and the conversation you are using never changes; you don't have to open the children.": "This doesnt really feel right to me.  I think what I imagine is that we just have one data type that is infinitely nestable. Right now we don't need to have KRs but we can have KRs. Waiter, it does. You're allowed to have children with KR. You're allowed to have a container that doesn't have KR that has children that do have KR.

These are called waves. Each wave can publish its own PRs and can publish as many PRs as it wants. It always has at most one current PR but it also has previous KPRs.  This should be relatively cheap to create, to close, and to open. It always has at most one runner on it

My question becomes: how does linear just not have this kind of notion of an infinitely nested or nestable hierarchy?"
Jack Heart: "we can remove KRs for now, that can come back later"
Jack Heart: "lets land 1997"
Jack Heart: "Can sub-issues have sub-issues in Linear?"
Jack Heart: "Okay can you maybe launch a research subagent to really get at:
- the nuances of what limits and what relationships
- what is sort of copywrite versus what is normalized
- how it works in Linear versus Asana
- whether there are any other competing models we should look at?"

Jack Heart (pasted, 2026-11-20): "I've been going back and forth with Fable in this and also in the chapter or in the work tree for removing chapters, in which it came up that it was unclear whether waves now can have many projects, zero projects, or only zero to one projects and I'm finding it really hard to reason about all the different things so I'm just going to try to ramble here and see if we can figure it out.
1. The first driving factor is that I am finding that work emergently scales into multiple areas, which is, of course, nothing new. I'm currently driving tasks that have lots of follow-up tasks and turning one big design into a lot more skit, small-scope designs, or sequences of releases. I'm kind of driving that all from one single task thread. and in my mind previously, a task was kind of like one single PR mostly although that was another thing that's kind of a little bit ambiguous: can you have multiple PRs or not? I've gone a little bit back and forth on that. Then waves were these long-standing areas of work that were mostly shared in the repo and this is showing me that I kind of want some hierarchy where I'm able to manage multiple workers in parallel or even manage workers that themselves manage multiple workers in parallel. That is required and I don't really want to introduce some sort of public-facing wave for every level of hierarchy then coming from the other side, I want this to work. I want to work well with outside trackers, namely most importantly Linear. I'd even be okay designing around almost exclusively Linear and then retrofitting onto Asana.

But a world where, I think, what I'm really coming around to is the idea that there's really this bifurcation or split in the task tracking: there's the humans' task tracking and then there are the agents and below. There should be a level at which they meet and basically you can have a meeting. A human can say, "Yeah I'm going to go do this," and then the human can start a thread for doing that with their agent. Maybe that can spread out into further hierarchy beyond that. I also want it to scale up to the idea that you can run a company where you have an entire top-level company agent, or at least a repo-level agent, that's kind of managing everything. It seems plausible to me. I think there's some ambiguity about whether that agent would then create subtasks that other humans could go grab. I think that's entirely plausible.

I do think we should sort of keep in mind this idea: the hierarchy goes from the human-facing tool to the agent orchestration tool. Mostly while you can file tasks at a sort of level higher than you, still, fundamentally, I'm not really that interested in what some people are doing with sharing agents across users. I don't think that makes sense really
[…]
but I still get kind of lost on what maps to what. Is a wave?

I do ultimately want to have this: in Loopflow it's sort of a single UI where you can integrate with outside trackers to pre-populate or give you something to map to for all of your different task organizing. Whatever organization comes from the outside world, we should bring in, adapt to, and work within.

You should be able to have sessions for "I want to chat about whatever infrastructure or engineering or the Swift UI or whatever," however your company organizes things. You should be able to filter tasks to those groups and all those things. You also should be able to further subdivide your work within that without necessarily gunking up the world for all your coworkers."
Jack Heart, on "It doesn't invent levels there": "I think it probably shoudl be able to create new projects or initiatives, but it hsoul mostly not doing this"
Jack Heart: "i guess maybe one key question is: what other tools try to integrate and adapt a company's slack channels , google docs  /notion, issue tracker into one sort of coherent view of the work as one large graph

I'm struggling to think about a world where you want to integrate with both Slack and Linear and come out with one coherent set of waves"
Jack Heart (pasted): "Okay I'm starting to think the model is that Loopflow has its own hierarchy of the work. There's a good API for mutating that hierarchy in a way that doesn't inherently affect any of the outside systems. There are good APIs for connecting each of those. For each level you can connect it to a linear team initiative, project, Slack channel, Google Doc, Google folder, Notion folder, whatever.  that these waves can be either personal or shared in the repo they have their own hierarchy I suppose. They don't really need to honor the hierarchies of the other systems. You can theoretically set it up so that the hierarchy in Loopflow is backwards from the hierarchy in some other system. That's fine still fundamentally they kind of should when you want to do things like post in chat or file a task in a project and then we can traverse the hierarchy. If there is no chat attached to here, you can go up until you find one. If there isn't one at all, even in the repo, then it fails"
Jack Heart (pasted): "Then we just make sure our CLI and API are really good so that, if you do have a robust setup in Slack or Linear or whatever, you can easily just tell your agent, "Hey make it all for me," and it just should work.

We don't need to have an automatic one. We can allow it to be sort of more manual because agents are capable of maintaining it.

I think the only system that really works is that Loopflow owns its own hierarchy and works really well with its own hierarchy and then has integrations, rather than Loopflow just follows"
Jack Heart: "i think we we need something like lf wave publish also"
Jack Heart: "and figure out how team members share canonical wave sets"
Jack Heart: "personal to shared" (read as the answer to: which did you mean by publish — a. personal to shared)
Jack Heart: "In this world are waves and tasks separate? I'm not sure they need to be. At the same time I think there are some questions: can you attach a task and a project to the same wave? Maybe you can  or maybe you can't"
Jack Heart (on: does publishing a Wave publish the Waves under it): "For number one I think, generally speaking, publishing the wave should only publish the one because you want to be able to make a main wave that you do want to share (but then have subwaves that are just for your personal doing whatever). It's a little less clear to me whether you would want to ever publish the tasks, but just collapse the subhierarchy in between"
Jack Heart (on the row "Goal, memory, settings | no | yes | something a Wave may have"): "Right. In this world I think things like memory and settings would need to climb the tree and so it makes sense to me that we would go ahead and allow tasks to have this. I will say that for the most part if you're really truly a task, then there's basically no point to storing memory forever for later. I think that kind of applies in other places as well so that's something to figure out"
Jack Heart (on the row "Holds other work | no (until #1986) | yes | any Wave"): "Right I think this line is one of the motivating changes here, right? It's not great to have a task and then, "Oh find out: oh I actually want to do follow-up work." How does that happen?"
Jack Heart quoted the row "Where it's stored | the store, travels live | a directory on main | still different" with nothing under it.
Jack Heart (on storage A/B/C): "Yeah I think I'm probably most interested in exploring whether maybe waves shouldn't be content in the repo. I think that, once you're already having, it could be sort of like the task tracker, where it's sort of live published and not part of a PR. I think that probably makes more sense anyway"
Jack Heart (on "No Loopflow, no Waves"): "This isn't physically impossible, right? It's just kind of awkward for them to know the exact address to look at"

Jack Heart (pasted): "Can you write up a design doc for me for this whole system, focused on what the user experience is? I want to start with:
1. You're a solo founder. You start with Loopflow on day one and you slowly scale it into sort of like a self-running company where you can zoom it. You can get involved at any level of abstraction.
Level two might be: you're a five-person startup that has a pretty big codebase already. You have some combination of Linear, Slack, Google Docs, whatever, and you want to try using Loopflow, sort of coordinated as a team

And then level 3 would be: you're working inside the context of a really big organization. You try adopting Loopflow for yourself. You want to do that in a way that works for you and really helps you bring in all the context that you want, but doesn't gum up the works for anybody or else.

Then you are like, "Hey this is working well for me," and you want to start coordinating or collaborating with some other people on your team within the large organization. Can you again do that in a way where you guys are collaborating and it just looks like normal collaboration to anybody not using Loopflow? To you guys who are using Loopflow, you're able to share everything you want to share, like graphs and memory and"
Jack Heart: "I have another agent doing the same thing so make sure not to cheat an look at thir design or collide with theirs. We will comare an cxontrast later."
Jack Heart: "ok, now compare yours to theirs"
Jack Heart: "in this world, would we expland the git task tracker into the git wave store, and then just always sync that and then have optional syncs to/from linear and asana?"
Jack Heart: "does this also change the answer about the machine hub as the record of truth for the store into the git trakcer?"
Jack Heart: "also, how does this work for personal vs shared waves if we have one global git master"
Jack Heart: "i dont really mind the privacy but i wonder about the scalability of it. i wonder if there is more of tree model where basically your local store has everyhign from the main store + your own stuff"
Jack Heart (on the two-layer model with personal re-placing): "the other thing about that is that it becomes really unclear at the UI level what other people mgiht be seeing"
Jack Heart: "one ground rule would be shared can contain private but not the other way around"
Jack Heart: "another question might be: at one point does a hosted SaaS solution make the UX much better? and if so, is there a line in the sand for the functionality we should build in this opensource version that is more targetting individual adopters and small teams with minimal hierarchy?"
Jack Heart (on "Hearing from Slack, Linear and Docs as things happen… Without one you poll"): "I think i wan this at level 2 though"
Jack Heart: "i think basically single player hub: in, multiplayer hub: probably better as saas"
Jack Heart: "Okay I'm starting to think that this model is maybe a little unconstrained. I can imagine companies where some teams use Linear and some teams use Asana and that could potentially work. But it also feels like having this, with the source of truth potentially being Git in some places, Slack or Asana in others, and Notion or Linear in some places, ends up being kind of confusing"
Jack Heart (on the Linear and Google Docs rows of the live-without-a-public-address table): "It occurred to me that I didn't really think about this before but we can just use CloudflareD to do this"
Jack Heart: "can we start to clarify what is \"locked\" in this design vs what is unclear/"
Jack Heart (on locked item 17, Loopflow owns its own tree): "where this sort of breaks down is like ... if you have a wave that is a linear project, shouldnt adding \"tasks\" to it mean adding those tasks to Linear?"
Jack Heart (on "Above any tracker connection, Waves are still Loopflow's own…"): "and what about initiatives or teams that may exist?

What should we enforce at the data model level, and waht should be manually sync'ed via integration systems? when and where shoul htat sycing appear at each of our levels"
Jack Heart: "lets take the time to look at each fo our previous levels and map out how we expect teh system to behave in these dynamic ways for each, and see waht likes up and what has tension across use cases and user expectations"
Jack Heart (on "Every issue belongs to exactly one team, and it must be one of its Project's teams"): "what team is it in if its in a project for multiple teams?"
Jack Heart (on "3. At most one runner, at most one current PR. 4. At most one tracker connection per Wave."): "At most one session and worktree also. we will need more lgoica about when to open and close these in a way that makes sense."
Jack Heart: "what do we gain from making waves=linear initiatives, teams? we would basically get like ... KRs if we ant them maybe, and then for teams maybe its somsort of access boundary?"
Jack Heart: "lets say for now we dont want any of that"
Jack Heart (on one session on a shared Wave: one in total, or one per person): "one per person, but a person doesnt need an active session with every shared wave necessarily"
Jack Heart (on "Runner and worktree: one for the Wave. So only one of those five can be running it at a time."): "Wait what, no this is wrong, these are also one per person"
Jack Heart: "current PR is per person as well"
Jack Heart: "a wave may have at most one ACTIVE runner on your machine in its worktree. cron jobs would work in their own separate work tree"
Jack Heart (on whose Machine runs a shared Wave's cron job): "1. not sure there is such a thing right now. crons are per machine, at least until you have a saas server where the crons would run"
Jack Heart (on laptop and mini each having a worktree for a Wave): "I think both could have them yeah"
Jack Heart (right after): "im not sure though. i guess i want only one machine to be active per wave f"
Jack Heart: "well, one task = one remote brnach = one pr. but theoretically possibly two different machines pushing to it"
Jack Heart: "i think we eshould maintain one worker and one (chat) session on the same machine. so each wave has a machine home of sorts"
Jack Heart (on home Machine from the Wave above; reaching a Wave homed elsewhere): "yeah thats right. we are going to need cross-machine steering"
Jack Heart: "steering can work across people through mechanisms like linear comments"
Jack Heart: "its possible we should be doing that all the time" (unclear which "that"; asked)
Jack Heart: "but right now we can do more local steering"
Jack Heart: "well the issue i had with 1986 is that it was not clear to me that its the right way to do hierarchty"
Jack Heart: "in the world where in loopfloweveryhign is just nested waves but can be linked to a task or a project or other thing..... it becomes unclear to me whether we want to do things like upgrade to project or use subissues"
Jack Heart: "let's just right a new plan from scratch, abandon  1986, and figure out wheter to start moving from tehre"
Jack Heart (on 1. grow from the Task record? 2. slice 1 from #1986 minus the Linear link, or clean?): "1) Agreed.  2) better to start over, but allowed to look as a reference"
Jack Heart: "i will say, im not sure the fan out im using, never sent to the tracker, makes sense. I have been starting with a task, finding its kinda big, and then deciding to launch more tasks. none of that is not sent to the tracker."
Jack Heart: "can you write your best thoughts to a scratch design doc and i am going to review with another agent"

## 2026-11-22

On one nestable work record and choosing Linear mappings by outcome: "yeah this sounds right"
Jack Heart: "a team would own a project would it not"
Jack Heart: "just your diagram showing only a task for a team collabing in a large org"
Jack Heart: "ok. what do you recommend in terms of a plan to start to test drive this"
Jack Heart: "what is slice 1"
Jack Heart (on auto-parenting `lf task create` from a Task thread/worktree): "This seems dangerous to me. i thikn making a subtask needs to be an explicit flag"
Jack Heart: "hmm, what do we get out of not usign subissues"
Jack Heart: "also, does linear require any kidn of alignment of project between issues and subissues"
Jack Heart: "What are the defaults on the autoclose"
Jack Heart (on Slice 1 parent sync: A sync to Linear sub-issues vs B Loopflow-only): "ok, lets do A then"
Jack Heart: "lets use this task for slice 1"
Jack Heart: "does linear refuse cycles? we should refuse cycles"
Jack Heart (on 1 edit this branch in place vs 2 reset to origin/main): "1"
Jack Heart (on concept-review Q1, completing a parent with children): "i think i do want to have compelte parent means complete/abandon children"
Jack Heart: "of course it shoul be clear thoughout the app"
Jack Heart (on concept-review Q1-Q4): "is either one better or worse than other" / "1 -- can we just abandon them 2 these shoudl probably usually be auto completed but if not then they get abandoned" / "3 b i think"
Jack Heart (on Q4 single-PR vs multi-PR on merged parent): "4 i am not sure about this one, lets talk it thorough"
Jack Heart: "all the other orchestrators assume multiple prs allowed per session/task i assume"
Jack Heart (on Q4 multi-PR on merged open Task): "i think we should just do that, but assume always 1 task = 1 worktree = 1 current pr"
Jack Heart: "i dont think we should leave inprogress children open. they should get abandoned too"
Jack Heart (on abandoning all remaining open children when completing a parent): "its possible this should be a setting like linear, but probably better to start this way."
