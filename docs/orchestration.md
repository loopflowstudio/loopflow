# Orchestration

You've launched a Skill and a Graph. This page makes them yours: which agent
runs each step, which machine runs the Graph, how a Task moves from idea to
landed PR, and how a Wave keeps going.

Loopflow ships reasonable Graphs for software engineering, and they are
templates. Copy one with `lf graph customize feature`, change it, or write down
the way you work and share it.

## Skills

A Skill is one job in Markdown: `design` plans, `implement` builds, `compress`
simplifies, `qa` verifies. Chain Skills in a Graph rather than cramming
everything into one prompt; steps pass work through Git commits and notes in
`scratch/`.

### Extend or replace a Skill

```bash
lf skill customize pr-message    # creates .lf/skills/pr-message.md extending lf:pr-message
lf skill customize qa --full     # copies the full builtin text to replace it
lf list skill                    # shows which Skills extend or replace others
```

Extending a builtin lets you add repo-specific instructions while still
receiving upstream improvements to the builtin:

```markdown
<!-- .lf/skills/qa.md -->
---
extends: lf:qa
---
## In this repository

Run `make e2e` against the local preview server before finishing.
```

You can also extend another Skill by name to build specialized variants on top
of your repository's version (or the builtin when no repository file overrides
it):

```markdown
<!-- .lf/skills/card-implement.md -->
---
extends: implement
agent: claude:opus
---
## Card rules

Verify every state transition against the spec before committing.
```

Frontmatter merges parent under child (child keys win), and the child's body
appends after the parent's. Use `extends: lf:<name>` to inherit Loopflow's raw
builtin directly and skip any repository overlay; using `extends: <name>` in a
file of the same name also falls through to the builtin. A Skill without
`extends:` replaces the builtin of the same name for everyone on your team.

### Automatic commits per step

When a Skill step edits files in a Graph, Loopflow commits them automatically
(and pushes on non-default branches):

```text
lf implement: Count retries per Task

The board read attempts from the wrong row.
```

The `commit-message` Skill writes commit messages (and `pr-message` writes PR
copy). Route them to a fast, cheap model:

```bash
lf set --global skill.commit-message.default_agent claude:haiku
```

Using `default_agent` keeps your `agents` failover list behind it if Haiku is
unavailable. A Graph’s `-a` selects its workers. Its automatic commit uses
the `commit-message` route on that Machine. Run the same helper manually
with `lf commit --step implement`.

Both writers get every changed path, its line counts (or a binary marker),
and a command to read one file’s patch. Patches under 15,000 tokens appear
in full. For larger changes, the writer chooses which files to inspect. It
can read a ten-line retry even when a 200 KB generated file comes first.

## Pick the agent per Skill or Graph step

Specify `agent:` (pinned) or `default_agent:` (with failover) in Skill
frontmatter, Graph steps, or config:

```markdown
<!-- .lf/skills/audit.md -->
---
agent: codex
context:
  docs: [scripts/testing.md]
---
Audit auth changes on this branch. Fix missing validation and untested paths.
```

```yaml
# .lf/graphs/ship-api.yaml
- step: {name: implement, agent: claude:opus}
- audit                         # runs on Codex per its frontmatter
- cmd: land --wait-and-fix
```

- `agent:` pins the harness (and model, if named).
- `default_agent:` sets the preferred starting point while letting headless
  steps fail over through your [`agents`](config.md#keep-going-when-a-provider-runs-out)
  list on usage limits or unavailable models.
- Passing `-a <harness>` to `lf graph` overrides every step in that Graph run
  (including nested Graphs and routers) without affecting unrelated child Tasks.
- `context.docs` in Skill frontmatter attaches extra files whenever that Skill
  runs.

See [Configuration](config.md) for the full precedence rules across personal,
repository, Wave, Skill, and Graph-step settings.

## Compose Graphs, loops, and branches

Run any Graph directly, headless, interactively, or on another Machine:

```bash
lf -b graph ship-api                     # headless end to end
lf -i graph feature                      # review at breakpoints in this terminal
lf --machine mini -b graph ship-api      # run on a remote Machine
lf graph ship-api --context --json       # preview first step's context without launching
```

Mix Skills and mechanical `cmd:` steps:

```yaml
# deliver (builtin)
- reflect                     # folds your feedback from this Task into skill edits
- cmd: land --wait-and-fix    # syncs, clears scratch, and merges when CI passes
```

### Loops (`loop:`)

A `loop:` block repeats when its deciding Skill returns `iterate`. A trailing
breakpoint lets you review that decision before the next pass. The built-in
`build` Graph puts implementation in an inner loop so the outer loop reviews
the whole change.

```yaml
# build (builtin)
- loop:
    - loop:
        - cmd: sync
        - implement        # decides: another planned pass (iterate) or ready for review (advance)
    - concept-review       # read-only: writes scratch/concept-review.html
    - realign              # decides: take review into plan and loop back, or move on
- compress                 # simplify what the branch touched
- qa                       # exercise the real change end to end
- pr-publish
```

Any Skill can be the deciding step of a loop: it returns `advance`, `iterate`
(with direction), or `blocked` (with a question).

During a Graph run, each `iterate` launches the loop’s Skills again in fresh conversations.
For work that needs several passes, `kickoff` groups it into 2–4 substantial
end-to-end passes, aiming for no more than 5. Within a pass, `implement` keeps
building while the next item is the same kind of work, and stops cleanly rather
than starting another pass when context is already heavy. Before iterating, it
collapses the finished pass in `scratch/plan.md` to one line with its commit and
names the next pass; if a counterexample breaks the plan, it notes what broke
and advances so review can replan.

Once planned passes are built, `concept-review` reviews the whole change
read-only (`scratch/concept-review.html`) and `realign` reconciles those
findings into `scratch/plan.md`—fixing small mismatches inline, deferring
separate outcomes under `## Deferred findings`, or regrouping remaining passes
and iterating back to `sync`. Aim for one diff a person can review in a sitting.
Around 2,000 lines of non-test change is a soft ceiling.

The deciding step sees which pass it is on and how many steps the loop
repeats, including the decision itself.

```text
Decision pass 3. Iterating repeats 2 steps, back to sync.
```

It keeps iterating past 5 when real work is left, and returns `blocked` when
passes shrink into small increments that no longer converge. `lf task status`
shows the reason the decider gave. If the step succeeds and the Graph then
fails, status shows the Graph error.

### Docs-first (`write-docs`) and QA playbooks (`.lf/qa/`)

- **`write-docs`** updates your actual README, guides, or reference docs right
  after planning so you can read and edit what is being built while
  implementation runs (in Loopflow Desktop, the edited files open in the Files
  pane automatically; elsewhere, their paths are printed in the conversation).
- **`qa`** maintains repo-local verification playbooks under `.lf/qa/`, one
  file per surface. The first line of each file states when to read it, so runs
  open only the playbooks relevant to their diff, and update them when a
  surface changes.

### Conditional routing (`xor:`)

`xor:` picks exactly one branch based on a router Skill's `{"path": "NAME"}`
output (an empty branch exits cleanly):

```yaml
- qa
- xor:
    router: triage
    paths:
      repair:
        graph: code
        description: "Reproduced defects within the change need repair"
      silence:
        description: "No actionable defect in the supplied change"
```

### Optimization loops (`optimize`)

```bash
lf graph optimize : "cut p50 export time on the 60k-row dataset"
```

`optimize` measures a baseline, plans one change, implements it, and keeps or
reverts based on the measurement until no clear improvement remains, then
compresses and re-measures. Compose it with `deliver` to ship the win:

```yaml
# .lf/graphs/speed-up.yaml
- graph: optimize
- graph: deliver
```

## Where a person takes part

```bash
lf task create "Compare two layouts for the Task list" --graph feature --run
```

`explore` runs first, on its configured agent, which can differ from the one
you talk to in the Task conversation. When it finishes, the Graph reaches a
**breakpoint**. Open the Task from **Now** in Desktop or run
`lf resume --task <ID>`. Its conversation links to the layouts in
`scratch/explore.html`, recommends one and presents numbered questions.
Ask why a layout was recommended and the Graph stays there. Reply “Use the
compact layout and keep the keyboard shortcuts” and planning starts with
that choice. To take the recommendation as written, run `lf task run <ID>`.

Put `- break` where you want to review what the preceding step produced.
The builtin `feature` Graph has two such stops.

```yaml
# feature (builtin)
- step: explore
- break
- graph: plan
- loop:
    - graph: build
    - step: ship-review
    - break                # review the PR before delivery
- graph: deliver
```

After `ship-review`, the conversation links to `scratch/ship-review.html`
and explains what ships and what was left out. If the worker recommends
delivery, you can still reply “Fix keyboard focus first.” The build loop
runs again with that feedback. Open the review with
`lf page open scratch/ship-review.html` or in Desktop Files, then use
`⌘↩` to send your choices to the Task. If a worker reports that it is
blocked, the reason stays visible until you resolve it. Delivery merges the
PR and completes the Task.

The built-in research Graph uses `step: research` to select the Skill with
the same name as the Graph. It runs once per pass. To repeat the investigation
with new direction, include the decision and your reply.

```bash
lf task run LOO-12 --decision iterate --reason "Compare keyboard use"
```

The Task conversation includes `--at-arrival` when it continues a review.
A reply from an earlier visit leaves the current visit available.

To give a breakpoint a name, write `- break: layout-choice`.
`lf task run <ID> --from layout-choice` goes directly to that marker.
`lf task run <ID> --from explore` runs the Skill again. A jump straight to
a breakpoint shows where continuing will take you. In an existing Graph,
`interactive: explore` means `explore` followed by a breakpoint. If your Task
was already waiting to start that Skill, its next run uses the captured Skill
and stops with its result. A run already in progress finishes in its original
Graph. Its recorded step numbers stay the same in history.

Run `lf explore` to develop the alternatives together in your terminal.
Run a Graph directly with `lf graph feature` to review its finished steps
at that terminal. A direct Graph run in a Task checkout leaves the Task at
its saved position.

### Build and customize review pages

Review skills (`ship-review`, `explore`, `concept-review`, `qa`, `demo`) write
body-only HTML fragments under `scratch/<name>.page.html` and compile them into
self-contained pages with `lf page build`:

```html
<!-- scratch/ship-review.page.html -->
<h1>Retry budget per Task</h1>

<h2>How it works</h2>
<lf-diff path="src/retry.rs" lines="40-68">
  <lf-note line="52">Counts attempts per Task ID rather than per process.</lf-note>
</lf-diff>

<h2>Findings</h2>
<lf-component name="decision" id="f1" title="Cap retry log growth" recommendation="File Task">
  <p>Long-running Tasks append one row per backoff tick without pruning.</p>
</lf-component>

<button data-lf='{"kind":"task.create","title":"Prune retry log rows"}'>File Task now</button>

<div data-question="q1" data-label="Rented Machine retry default">
  <p>1. Keep the 3-attempt default for rented Machines?</p>
  <textarea></textarea>
</div>
```

```bash
lf page build scratch/ship-review.page.html              # → scratch/ship-review.html
lf page build scratch/ship-review.page.html --base main  # override diff base
lf page open scratch/ship-review.html                    # open in browser or Desktop Files
```

Source files contain body HTML only—no `<html>`, `<head>`, `<style>`, or
`<script>`. `lf page build` wraps the body with the theme CSS, runtime script,
and a JSON work stamp (work target, checkout, base/head, build hash), and
expands:

- `<lf-diff path="…" [lines="40-68"]>`: real hunks from Loopflow's diff engine
  (including untracked files and renames; omit `lines` for every hunk in the
  file). Add `<lf-note line="52">` (new line) or `<lf-note old-line="48">`
  (deleted line) children; readers can toggle line notes off.
- `<lf-file path="…" [lines="1-40"]>`: numbered lines as they stand on disk,
  with optional `<lf-note line="…">` children, for unchanged code.
- `<lf-component name="…">`: expands `.lf/components/<name>/template.html` (and
  optional `style.css`), overriding builtins. Substitutes HTML-escaped
  `{{attr}}` attributes and `<slot>` / `<slot name="…">` children. Built-in
  components are `decision` (`id`, `title`, `recommendation`, body slot) and
  `before-after` (`before` and `after` slots).
- `data-choice`, `data-multiselect`, `data-question`: interactive controls
  included in the `⌘↩` reply.
- `data-lf='<partial JSON>'`: expands partial JSON into a strict Machine
  `Operation` using the build checkout (injecting `directory` and per-click
  `request_id` where needed) and validates it at build time.

`lf page build` fails—naming the element and line—on any missing path in
`<lf-diff>`, `<lf-file>`, or `[data-path]`, any `<lf-diff>` path or range not
present in the diff, any unknown 7–40 hex commit SHA inside `<code>` (add
`data-no-check` to opt out), any invalid `data-lf` operation, or any `<script>`
or `<style>` tag in the source.

Customize review pages for your repository without forking builtins:

- **Theme:** add `.lf/theme.css` setting CSS custom properties for light and
  dark modes; `lf page build` uses it in place of the built-in theme.
- **Components:** add `.lf/components/<name>/template.html` (and optional
  `style.css`) to define new `<lf-component>` blocks or override `decision` /
  `before-after`.
- **Skill rules:** extend a built-in review skill with `extends: lf:ship-review`
  (or `explore`, `concept-review`, `qa`, `demo`) in `.lf/skills/` to add
  repository-specific sections or checks while keeping the base skill.

Run `lf help page` for the full authoring contract, and see
[Review pages](surfaces.md#review-pages) for how pages behave in browsers,
Desktop, and offline.

### Built-in Graphs

```bash
lf task run LOO-13 quick    # builds from the brief, runs qa, and lands when CI passes
```

| Graph | What it does |
|---|---|
| `feature` | Choose a direction after `explore`, then review the built change after `ship-review` |
| `code` | Same as `feature`, starting at `plan` (default when no Graph is configured) |
| `quick` | loop(`sync` → `implement`) → `qa` → `land --wait-and-fix` |
| `pursue` | `plan` then `build` from request to published PR |
| `research` | Review the findings after each research pass and decide whether to investigate further |
| `optimize` | Iterative measure → change → keep/revert loop |

`quick` takes a small, clear change straight from the brief to a merged PR with
no breakpoint. Desktop's Start menu marks it **merges without asking**, and
`lf task auto off <ID>` still pauses before `land`. A brief bigger than one
bounded change stops `quick` at `implement`, blocked, before anything is built.

Copy any builtin to customize it for your team:

```bash
lf graph customize feature                # writes .lf/graphs/feature.yaml
lf --wave product set wave.graph feature  # personal override without committing
```

### Describe a Graph

```yaml
# .lf/graphs/ship-api.yaml
description: Build the endpoint, audit it, then deliver.
steps:
- implement
- audit
- demo
- break
- graph: deliver
```

```console
$ lf graph list
code       Build and deliver. You review the change before it merges.
my-graph
ship-api   Build the endpoint, audit it, then deliver.
```

`description:` is one line saying what running the Graph gets you. Desktop's
Start menu shows it beside the Graph's name and counts the breakpoints. A
Graph written as a bare list still runs and appears under **More ›**.

This command shows the workflow from Desktop’s New Task sheet, including
breakpoints in composed Graphs and branches.

```bash
lf graph list --json | jq '.[] | select(.name=="code") | .workflow'
```

A Graph whose definition cannot load has a null `workflow`.

### Auto

Auto is a setting on the Task: on, off, or never set. While it is on, the
Graph follows each worker’s proposed continuation through breakpoints and may
merge the PR. Turning on **Auto** in Desktop also starts the Task.

```bash
lf task auto on LOO-12              # continue through breakpoints and allow merge
lf task status LOO-12               # LOO-12  moving · auto: on by Maya, Nov 18 09:04
lf task auto off LOO-12             # LOO-12: auto off
```

- Turn auto on while `build` runs and the Graph takes the recommendation
  when it reaches the breakpoint after `ship-review`.
- `lf task auto off` kills nothing. The run waits at its next breakpoint,
  or before a merging step (`lf land`, `lf pr arm`, `pr-land`): "Waiting at pr
  land: auto is off (Maya). `lf task auto on INF-1` lands it." Auto-merge that
  Loopflow armed is disarmed.
- Off stands: only `lf task auto on` turns it back on; an `auto: on` in
  configuration never does. `lf -b task run` crosses breakpoints for that invocation only.
  With auto off it still waits at the landing step.
- Never set, a run that reaches a merging step lands; `lf task run` says so on
  its `merges:` line. A failing run leaves auto alone.
- `auto: on` beside `graph:` in a Wave's config turns auto on for every Task
  that takes the Graph up. Reopening a Task clears it.

### Failure and retries

- A failed step is tried again, up to three attempts, starting at that step (earlier steps stay done).
  Pre-launch failures (such as a missing harness CLI) fail immediately so you
  can fix them and run `lf task run`.
- If a headless OpenCode or Pi agent asks a question, the run stops and prints
  it; answer with `lf steer LOO-12 "…"` and continue with `lf task run LOO-12`.

| Task state | Meaning |
|---|---|
| `unstarted` | No worktree yet |
| `idle` | Checked out with no run underway or decision pending |
| `active` | Needs attention, including a Task at a breakpoint |
| `moving` | Graph running, whatever the auto setting |
| `paused` | Started Task held by [`lf task pause`](work.md#pause-a-task): Graph held, conversation available, ownership and worktree kept |
| `done` / `abandoned` | Completed / dropped |

## Run Tasks on pools or rented machines

```bash
lf task run LOO-12 --pool lab --rent code --quote   # preview placement and max cost
lf task run LOO-12 --pool lab --rent code           # run on a free `lab` box or rent from `code`
```

When the run ends, its Sessions and Graph history copy back to your machine
before any rented box is returned. See [Machines](ops.md#machines).

## Wave Goals

A Wave's `.lf/wave/<name>/GOAL.md` is the operating brief for its conversation
and Tasks:

```markdown
<!-- .lf/wave/infra/GOAL.md -->
## Objective

Keep the runtime architecture legible. Each pass: read the Wave's Tasks and
memory, pick the next useful move, and fold what shipped into memory.

## Process

Make mechanical changes directly; write a scratch design first when the
change crosses storage, auth, or public APIs.
```

Next: [Troubleshooting](troubleshooting.md): what survives crashes and how to
diagnose anything that stops.
