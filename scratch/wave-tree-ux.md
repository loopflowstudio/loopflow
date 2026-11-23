# Wave Tree: User Experience Design

A design for Loopflow's work hierarchy, organized around one recursive noun
(**Wave**) and four stages of adoption—from a solo founder on day one to a
pocket team inside a large company.

Related notes: `scratch/wave-tree.md` (core invariants),
`scratch/hierarchy-research.md` (tracker facts), `scratch/plan.md` (the
narrower parent-Task slice already built on this branch).

---

## The Model in One Page

Loopflow owns **one tree of Waves**.

A **Wave** is a channel and a scope of work. Every node in the tree has the
same shape, whether it represents a whole company, a product area, a project,
or a single pull request:

- **Conversation(s)** — always yours. A Wave's conversation can do work
  directly, conduct child Waves (hearing when one stops, fails, or lands), or
  both.
- **Work of its own (optional)** — a checkout, a Graph run, and PRs over time
  (at most one active PR at a time). If one effort turns into five PRs, it
  spawns five child Waves.
- **Children (optional)** — child Waves, nested as deeply as the work needs.
- **Connections (optional)** — links to outside systems: a Linear Team,
  Initiative, Project, or Issue; a Slack channel; a Google Doc or folder; a
  Notion page or folder.
- **Process & memory (optional)** — a default Graph, skills, and standing
  notes.
- **Visibility** — **personal** (local to you) or **shared** (checked into the
  repo or a shared team layer).

Three rules keep the whole system simple:

1. **Mutating Loopflow's tree never mutates outside tools.** Creating,
   nesting, splitting, or abandoning Waves stays in Loopflow. Pushing something
   out—filing a Linear issue, creating a Project, posting in Slack—is always an
   explicit action.
2. **Everything missing resolves by walking up.** Need a default Graph, a
   Slack channel to post to, a Linear Project to file under, or reference docs?
   Loopflow checks this Wave, then its parent, all the way to the root. If
   nothing along the path has one, the action fails cleanly and says why.
3. **Structure and process are shared; agents are personal.** Two teammates
   can open the same shared Wave and inherit the same Graph, memory, and
   external connections, while each person's conversation and personal child
   Waves remain theirs.

> **Naming note:** This doc uses **Wave** for every node in the tree (what
> used to be a Task is simply a leaf Wave with a checkout and a PR, which can
> grow children whenever it needs to). Whether the UI still calls a leaf a
> "Task" or uses "Wave" everywhere is an open naming choice, not a model split.

---

## Level 1: Solo Founder — Day One to Self-Running Company

You start with a fresh repo and no external tracker. You want zero ceremony on
day one, and no ceiling a year later.

### Day 1: Flat and immediate
There is one root Wave for the repo. You open a conversation and ask for a
feature. Loopflow spins up a child Wave with a checkout, runs the default
Graph, and opens a PR. You don't configure a hierarchy, a tracker, or a
process.

### Week 3: Emergent fan-out
One Wave ("Billing") turns out to be a design doc, a schema migration, a
Stripe integration, and a UI pass. Inside Billing's conversation, you say
"split this up and run it."

Billing spawns four child Waves—each with its own checkout and PR—while
Billing itself stays open as the conductor. Its conversation doesn't nag you
on every passing step; it wakes up when a child hits a review gate, fails, or
lands. You steer from Billing, or click into any child to review its diff
directly.

### Month 4: Standing areas and processes
Patterns repeat. You create standing Waves under the root—`product`, `infra`,
`growth`, `support`—and give each its own default Graph and notes:
- `infra` defaults to a Graph with a migration and rollback check.
- `product` defaults to design → implement → demo.
- `support` connects to a customer-feedback folder and files bugs into
  `product`.

Any child Wave created under `infra` inherits `infra`'s Graph automatically.

### Year 1: Zooming in and out of a self-running company
Your tree now looks like a living org chart of work:

```
acme (root)
├── product
│   ├── onboarding
│   │   ├── invite-flow        [PR #412]
│   │   └── empty-state        [PR #415]
│   └── billing
├── infra
│   └── postgres-17            [PR #409]
└── growth
    └── launch-posts
```

Because every level is the same noun, **zooming in and out is seamless**:
- **Zoom all the way out** (`acme`): "What's blocked this week? Pause growth
  and put capacity on onboarding." The root conversation reads its children
  and redirects them.
- **Zoom to the middle** (`onboarding`): Steer the sequence of PRs, review the
  product arc, decide what ships together.
- **Zoom all the way in** (`invite-flow`): Look at the code, run tests, tweak
  a line, land the PR.

You get involved at whichever altitude needs your judgment, and let the layers
below run themselves.

---

## Level 2: Five-Person Startup — Coordinating as a Team

You have a real codebase, five engineers, and existing tools: Linear (Teams,
Projects, Issues), Slack (`#eng`, `#product`, `#incidents`), and Google Docs
for specs. You want Loopflow to coordinate the team without forcing anyone to
abandon those tools.

### Onboarding without a migration
Nobody moves their backlog out of Linear. Instead, one engineer asks an agent
to inspect the repo, Linear, and Slack and propose a starter tree of **shared
Waves** (checked into `.lf/wave/`):

```
repo (shared)               ── Linear: Engineering Team · Slack: #eng · Graph: code
├── swift-ui (shared)       ── Slack: #swift-ui · Docs: Design System · Graph: ui-feature
├── backend (shared)        ── Slack: #backend · Graph: backend-safe
└── q2-auth (shared)        ── Linear: Project "Q2 Auth" · Docs: Auth RFC
```

There is no brittle 1:1 mirror or background sync daemon fighting Linear. The
shared tree captures the team's standing areas, current projects, default
Graphs, and where to look for context. An agent can refresh or reshape it on
request as projects start and finish.

### Daily work: shared process, private execution
1. **Picking up work:** Alice grabs Linear issue `ENG-412` ("OAuth token
   refresh") under `q2-auth`. That opens a **personal Wave** under `q2-auth`,
   linked to `ENG-412`.
2. **Inherited context and process:** Without configuring anything, Alice's
   Wave inherits `q2-auth`'s Auth RFC doc, `repo`'s `code` Graph (or a
   `q2-auth` override), and `#eng` as the chat channel.
3. **Private fan-out:** `ENG-412` turns out to need three separate PRs. Alice's
   agent spawns three **personal child Waves** underneath it. Linear is not
   cluttered with sub-tickets nobody else cares about; coworkers just see
   `ENG-412` moving and clean PRs landing.
4. **Filing outward when it matters:** While implementing, Alice's agent spots
   an unrelated race condition in session storage. Alice tells it to file a
   Linear issue. Loopflow walks up from Alice's Wave, finds `q2-auth`'s Linear
   Project binding, and files the issue there for the team.
5. **Posting updates:** When `q2-auth` hits a milestone, "post a summary"
   walks up to the nearest Slack binding (`#eng`) and posts there.
6. **Encoding team norms:** When the team decides every backend PR needs a
   query-plan check, they update the Graph on the shared `backend` Wave once.
   Everyone's next Wave under `backend` picks it up.

---

## Level 3: Solo Adopter in a Large Organization — High Context, Zero Footprint

You work at a 500-person company. Engineering uses Linear or Jira with rigid
PM workflows, there are dozens of Slack channels, specs live in Google Drive
and Notion, and nobody else uses Loopflow.

You want all of that context in your agents—and you must not leave a trace on
anyone else's workflow.

### Everything starts personal
Your entire Wave tree lives locally on your machine. Nothing is committed to
the repo (`.lf/` isn't touched), and nothing writes to Slack, Linear, or Docs
unless you explicitly ask.

### Shaping the company's world to fit your head
Your Loopflow tree doesn't have to match the company org chart. You arrange it
around *how you actually work*, attaching external connections as read context
wherever they help:

```
my-work (personal)
├── checkout-team            ── Slack: #checkout-eng (read) · Docs: Checkout Drive Folder
│   ├── q3-payments          ── Linear: Project "Q3 Payments" · Docs: Payments PRD
│   │   └── PAY-881          ── Linear: Issue PAY-881
│   │       ├── schema       [PR #1820]
│   │       ├── retry-worker [PR #1824]
│   │       └── metrics      [PR #1829]
│   └── tech-debt            ── Graph: small-refactor
└── oncall                   ── Slack: #payments-alerts (read) · Docs: Runbooks Notion · Graph: incident
```

Notice what happens here:
- `checkout-team` pulls in `#checkout-eng` and the architecture Drive folder as
  context. Any Wave underneath it can read them without you copy-pasting links
  into every prompt.
- `PAY-881` fans out into three parallel PR Waves on your machine. Your PM
  never sees three mysterious sub-tickets; they just see `PAY-881` linked to
  your PRs.
- `oncall` uses your personal `incident` Graph and has the runbook folder
  attached, completely independent of how Linear is organized.

### What coworkers see
Normal GitHub PRs. Normal comments on `PAY-881` when you choose to update it.
Normal Slack replies when you choose to send them. Zero `.lf/` noise in diffs,
zero bot spam in channels.

---

## Level 4: A Pocket Team Inside the Large Organization

Your personal setup works so well that three teammates on Checkout want in.
The other 496 people at the company still don't use Loopflow—and shouldn't
have to know or care that you four do.

### Sharing without forcing the whole org
The four of you share a **team Wave layer**—either checked into a directory
your team owns in the repo, or (if the monorepo won't accept `.lf/` files yet)
a small shared team repo mounted as an overlay on your checkouts.

Inside that shared layer, you publish the Waves you want in common:
- `checkout-team` (with `#checkout-eng`, the Checkout Linear Team, architecture
  docs, and shared team memory: deploy gotchas, service boundaries, review
  norms)
- `q3-payments` (with the PRD doc, the Linear Project binding, and the team's
  `payments-safe` Graph)
- `oncall` (with the runbook folder and the team's `incident` Graph)

### Rich collaboration inside, invisible outside
- **New teammate ramp-up:** Teammate #4 opens Loopflow, pulls the shared
  layer, and immediately has the team's Graphs, memory, doc links, and channel
  bindings—without re-wiring anything.
- **Shared memory compounds:** When Engineer A discovers a subtle idempotency
  rule in the payments gateway and saves it to `checkout-team`'s memory,
  Engineer B's agent knows it the next morning under any child Wave.
- **Personal space stays personal:** Underneath the shared `q3-payments` Wave,
  each of the four engineers still has their own personal Waves and private
  sub-waves for active work. Nobody is fighting over a shared agent session.
- **Handoffs go through the company's tools:** When Engineer A's Wave uncovers
  a follow-up for Engineer B (or for a sibling team not on Loopflow), it files
  a normal Linear issue via `q3-payments`'s binding. To Engineer B, it's a
  clickable item they can open a Wave on; to the PM and sibling teams, it's
  just a normal Linear ticket and normal GitHub PRs.

To the rest of the organization, nothing changed—your team just ships cleaner
PRs faster. To the four of you, you share a living tree of Graphs, memory, and
context.

---

## Core UX Primitives

Across all four levels, a user only learns six ideas:

| Action | What it does | Touches outside tools? |
|---|---|---|
| **Open / zoom** | Enter any Wave's conversation, see its children and PR | No |
| **Spawn child** | Break work down into a child Wave (personal by default) | No |
| **Connect** | Attach a Linear object, Slack channel, doc/folder, or Graph to a Wave | No (links only) |
| **Run / conduct** | Run a Graph on a Wave; its parent hears when it stops, fails, or lands | GitHub PR when shipping |
| **Share** | Promote a personal Wave into the shared repo/team layer (this Wave only, not its personal children) | Git / shared team layer |
| **Surface** | Explicitly file a Linear issue/Project or post to Slack, resolved by walking up the tree | Yes (explicit) |

The key distinction is **Share vs. Surface**:
- **Share** makes a Wave's structure, default Graph, memory, and bindings
  available to *Loopflow teammates*.
- **Surface** creates or posts an artifact in *an outside tool* (Linear,
  Slack) for *everyone*, whether they use Loopflow or not.

Keeping those two verbs separate is what makes Levels 3 and 4 possible.

---

## Open Decisions to Settle Before Building

1. **One noun in the UI too, or "Wave" + "Task"?** The engine has one
   recursive node. Should the UI call everything a Wave (with a PR icon when
   it has a branch/PR), or call leaf PR nodes Tasks?
2. **Team overlay for Level 4.** When a pocket team can't commit `.lf/` to a
   company monorepo, where does their shared layer live? (A second git repo
   mounted as an overlay is the simplest fit with existing `.lf/` files.)
3. **Read vs. write permissions on connections.** In Level 3, connecting
   `#checkout-eng` as *read-only context* prevents an agent from accidentally
   posting to a 200-person channel when walking up the tree. Should
   connections default to read-only until write is enabled?
4. **Multi-repo root.** Level 1's "self-running company" and Level 4's
   multi-service team eventually span more than one repo. Does the top of the
   tree live above repositories (a workspace/company root), with repos attached
   at or below it?
5. **External-to-local updates.** When a Wave is linked to a Linear Issue (or
   Project), what flows *in* automatically (e.g. description edits, status
   changes, comments) vs. on demand?
