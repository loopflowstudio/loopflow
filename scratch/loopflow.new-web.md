# Loopflow: an instrument for individual and shared practice

Local website draft · 2026-10-01 · Direction from Jack Heart; final wording and layout remain under review. Current rendered copy lives in `website/content.yaml`.

## What to build

Explain Loopflow as a customizable instrument for working with coding agents and a shared company toolset for developing that practice, supported by six concrete feature explanations.

Jack's intent:

> there are a bunch of things that many people have done on their own but Loopflow does *all* of them and builds a higher-level system that can rely on them all working together

> And for each one good to be like: Here's the eli5 for what our approach is on this subsystem

Jack added two explicit positioning requirements:

> gives you one customizable, reliable workflow to work with agents in whatever way you want, with as much organization and computing around that out of mind.

> we are not incentivized to point you towards a certain set of models, and we are not trying to get you "in our system"

> gives your company a shared language and toolset for exploring and defining how to work with coding agents

Placement unresolved; no Wave was supplied. This is one coherent website increment: pitch plus supporting features. Broader catalog and demos can follow independently. The positioning and six subsystem choices are Jack's direction; wording and placement below remain proposals.

## Positioning and pitch — draft

**Loopflow is a software instrument.**

**Make software through agents, your way.**

Jack reaffirmed the instrument opening and explored “through agents,” “in your own way,” and “your own unique timbre.” The plain second line above is the current local proposal, not a final wording decision.

Work closely with an agent, hand off a task, or coordinate several streams of work. Shape the workflow around how you want to build. Loopflow takes care of the branches, context, checks, and handoffs that keep it moving.

**Choose your models. Own your process.**

Loopflow's job is to make your way of working dependable. We have no stake in which model you choose. Use your own agent accounts, run on your own machines, and keep your skills, workflows, and memory in files you can read, change, and version. Loopflow is open source.

**Develop your company's way of working with agents.**

Give people a shared language and toolset for trying ideas, comparing approaches, and keeping what works. A useful instruction can become a shared skill. A successful sequence can become a workflow. Teams can review and improve those files together while choosing the agents and level of involvement that suit the work.

The features below explain what makes those promises concrete.

### Meaning and evidence

“One workflow” means one coherent way to organize and operate work, customizable into many actual workflows. Avoid implying a mandatory sequence or forcing organization onto a single conversation. “Reliable” points to retained work, recorded progress, explicit reviews, and recoverable handoffs; it does not guarantee model output.

Model neutrality is Jack's stated product/business intent. Provider adapters, independent accounts, local operation, editable files, and the MIT license provide concrete supporting evidence. They do not establish seamless conversation migration between arbitrary models or universal workflow portability. The pitch should express the intent and demonstrate these choices without claims about competitors' motives.

The company promise is shared experimentation and learning: people can name, inspect, discuss, version, and improve how they work. It does not require everyone to use an identical workflow or imply a new hosted company administration product.

## Exact section copy — draft

### Features

**The pieces of your agent workflow, working together.**

Worktrees, code review, project plans, credentials, workflows, and memory. Loopflow connects them so each stage can pick up where the last one left off.

#### Worktree & Git management

**Give each task its own place to work.**

A worktree is a separate working copy of your project. Loopflow creates or reuses one for each Task and keeps track of its branch and pull requests. Several tasks can move forward at once. When it's time to bring changes together, Loopflow handles syncing, delivery, and cleanup.

[Read about delivery →](/architecture/delivery)

#### CI & PR review

**Check the work, review it, and send it back when it needs another pass.**

Loopflow has workflows for checking changes, preparing pull requests, and walking you through the result. Review feedback can send the work back through implementation. When a Task's PR is handed off for landing, the CI watcher can start a repair if its required checks fail.

[Read about review workflows →](/docs/authoring)

#### Project management

**Keep the plan connected to the work.**

Projects and Tasks live in Linear. Loopflow connects a Task to its working copy, conversations, workflow, and pull requests. You can see what is moving, what is waiting, and what needs a decision. Longer-running goals live in Waves, which keep their purpose and memory as plans change.

[Read about Tasks and Waves →](/docs/waves)

#### Credential forwarding

**Use your accounts on another machine without setting them up there again.**

Start work over SSH and Loopflow can use accounts from your laptop. For Claude and Codex, it borrows a short-lived access token when the remote agent needs one; it doesn't copy your saved login to that machine. Background work that outlives the connection uses accounts installed on the remote machine.

[Read about remote access →](/docs/security#understand-account-authority-over-ssh)

#### Customizable skills & workflows

**Write down how you work. Run it again. Change any step.**

A skill is a text file with instructions for an agent. A workflow connects skills, commands, and review stops. Loopflow includes a library for design, implementation, debugging, review, and shipping. Edit the files to make it yours—including when work should repeat and when it should wait for you.

[Read about skills and workflows →](/docs/authoring)

#### Memory & context

**Leave useful notes for the next round of work.**

The current change keeps its working notes beside the code. A Wave keeps longer-lived goals and lessons in files you can read and edit. Loopflow brings the relevant notes into later work, and lets you inspect how much context it sends to the agent.

[Read about memory →](/docs/waves)

### What working together buys you

A Task in your plan gets a working copy. Its workflow brings in the relevant instructions and memory, builds the change, and stops for review. Feedback can send it through another pass. When the PR is handed off for landing, CI repair returns to that Task's checkout. Lessons can be saved for the next task.

**The connections are part of the product, too.**

## The demo

Website: understand the individual and company promises, follow Features from navigation, recognize the six subsystems, and follow working documentation links. The connecting example explains why the pieces belong together.

Later product demonstration: follow one real Task from Linear through checkout, review revision, PR checks, and a saved lesson. Show an interruption and return to the same work. Credential forwarding deserves a separate remote demonstration rather than forcing every feature into one story.

## Current system and proposed cut

`website/content.yaml` owns copy. `website/main.py::build_homepage()` currently renders the hero, three-step overview, problem/solution grid, install, technical definition, and file examples. `Navbar()` has no Features link. CSS and browser/accessibility tests already exist.

Proposed order: instrument hero and individual promise → three-step example → model independence and ownership → Features and connecting example → company practice → install → technical definition and editable examples. Replace the overlapping problem/solution grid with Features and the generic company-scale paragraph with the shared-practice pitch. Add `/#features` to navigation, including from docs. Use readable headings and links with visible mobile text and the existing typography. No new page or interactive catalog is needed for this increment.

## Data structures and key functions

Use the existing hero and scale copy owners for the individual and company promises; move the scale renderer to the proposed position. Add `homepage.independence` for model choice and ownership. `homepage.features` holds heading, introduction, items, and connecting example. Each item has `title`, `summary`, `description`, `href`, and `link_label`. These are authored site content, not runtime DTOs.

Add `FeaturesSection()` beside existing FastHTML components; `build_homepage()` places it and `Navbar()` links to its stable ID. Keep copy in YAML and use existing content validation. No feature registry, CMS, database, or runtime dependency.

## Delete — do not maintain

If this replacement is accepted: remove `homepage.problems`, `PROBLEMS_CONTENT`, the problem-grid renderer, and exclusively used `.problems-*` / `.problem-item` CSS in the same cut. Replace the old `homepage.scale` wording and remove its old placement inside the technical definition. Replace exclusive test expectations. Preserve the three-step overview and its tests.

## Constraints and forbidden outcomes

Claims must match the published version at implementation time. Current-head evidence is not release or live-demo proof. CI repair requires the applicable landing handoff; worktrees do not eliminate integration conflicts; SSH sends credentials into a trusted remote environment. Do not promise universal autonomy, guaranteed correctness, sandboxing, or that competitors lack these parts. Avoid reviving retired daemons, product names, or watch/listen modes from historical copy.

## Internal slices and follow-ups

**This slice:** review the local website draft's pitch, subsystem explanations, and layout. Jack's “lets continue” authorized the local draft; publication remains outstanding. The renderer, navigation, responsive layout, and focused checks are in place. The old problem grid and generic company paragraph have been replaced; no further deletion is planned for this slice.

Independent follow-ups: a broader features page covering Sessions, account routing, Desktop/CLI, monitoring, and scheduled work; a recorded Task walkthrough; a credential-forwarding walkthrough. Each gets its own design when requested.

## Done when

Gate: `cd website && uv run python dev.py test` passes, including Features navigation from home/docs, six accessible entries with resolving links, and mobile layout. Test behavior and structure, not exact prose. Review judges whether the approach to each subsystem is understandable and the connecting example earns its claim.

Check: 2026-10-01 — `cd website && uv run python dev.py test -k 'homepage or navigation or Mobile or Accessibility'`: 54 passed, 3 expected mobile hidden-title skips; revised hero: 4 focused checks passed; Ruff lint passed; desktop/mobile captures inspected. Full gate remains deferred; existing main.py formatting differs from the formatter.
