# Orchestration

You've launched a Skill and a Graph. This page makes them yours: which agent
runs each step, which machine runs the Graph, how a Task moves from idea to
landed PR, and how a Wave keeps going.

Loopflow ships reasonable Graphs for software engineering, and they are
templates. Copy one with `lf graph customize feature`, change it, or write down
the way you work and share it.

## Skills

A Skill is one job in Markdown: `design` plans, `implement` builds, `qa`
verifies, `gate` reviews and publishes. Chain Skills in a Graph rather than
cramming everything into one prompt; steps pass work through Git commits and
notes in `scratch/`.

### Extend or replace a Skill

```bash
lf skill customize pr-message    # creates .lf/skills/pr-message.md extending lf:pr-message
lf skill customize gate --full   # copies the full builtin text to replace it
lf list skill                    # shows which Skills extend or replace others
```

Extending a builtin lets you add repo-specific instructions while still
receiving upstream improvements to the builtin:

```markdown
<!-- .lf/skills/gate.md -->
---
extends: lf:gate
---
## In this repository

Run `make e2e` before publishing.
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
- gate
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
lf -i graph ship-api                     # every step interactive
lf --machine mini -b graph ship-api      # run on a remote Machine
lf graph ship-api --context --json       # preview first step's context without launching
```

Mix Skills and mechanical `cmd:` steps:

```yaml
# deliver (builtin)
- reflect                     # folds your feedback from this Task into skill edits
- gate                        # publishes PR so CI starts, then reviews while CI runs
- cmd: land --wait-and-fix    # merges when checks pass, repairing CI failures
```

### Loops (`loop:`)

A `loop:` block repeats when its final step returns `iterate`. The built-in
`build` Graph puts implementation in an inner loop and design review in an
outer loop, then runs `compress` and `qa` once after the design settles.

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

Mark any step `interactive:` to pause for human input; every other step runs
headless. Built-in `feature` puts you in the loop at two high-leverage moments:

```yaml
# feature (builtin)
- interactive: explore      # choose what to build
- graph: plan               # kickoff, concept-review, realign, write-docs
- loop:
    - graph: build          # sync/implement, concept-review, realign, compress, qa, publish PR
    - interactive: ship-review  # read one summary page; iterate sends feedback back to build
- graph: deliver            # reflect, gate, land --wait-and-fix
```

```bash
lf task run LOO-12 feature          # stops at explore; opens in the Task conversation
lf task run LOO-12                  # continues from where it stopped
lf task run LOO-12 --from explore   # jump back to any step (by id, Skill, or sub-Graph)
lf -b task run LOO-12               # run headless through all remaining steps
lf task auto on LOO-12              # auto on: an agent stands in for you at every remaining step
```

At **`ship-review`**, Loopflow builds one comprehension page
(`scratch/ship-review.html`) from `scratch/ship-review.page.html`: what was
built, how it works on real diffs, the key data structures and APIs, the
trade-offs and invariants to know going forward, findings as `Fix here` /
`File Task` / `Drop` choices, and numbered questions—without passing-test
counts or routine CI status. Open it with `lf page open scratch/ship-review.html`
(or in Desktop Files), highlight text to comment, pick your choices, and press
`⌘↩` to steer the Task directly, or move on to `deliver`, which merges the PR
and completes the Task.

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
lf task run LOO-13 quick    # builds from the brief, runs qa and gate, and lands when CI passes
```

| Graph | What it does |
|---|---|
| `feature` | `explore` (interactive) → `plan` → `build` ↔ `ship-review` (interactive) → `deliver` |
| `code` | Same as `feature`, starting at `plan` (default when no Graph is configured) |
| `quick` | loop(`sync` → `implement`) → `qa` → `gate` → `land --wait-and-fix` |
| `pursue` | `plan` then `build` with no interactive stops: request to published PR |
| `research` | Investigate and report findings without opening a PR |
| `optimize` | Iterative measure → change → keep/revert loop |

`quick` takes a small, clear change straight from the brief to a merged PR with
no interactive step—Desktop's Start menu marks it **merges without asking**, and
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
- interactive: demo
- graph: deliver
```

```console
$ lf graph list
code       Build and deliver. You review the change before it merges.
my-graph
ship-api   Build the endpoint, audit it, then deliver.
```

`description:` is one line saying what running the Graph gets you. Desktop's
Start menu shows it beside the Graph's name, with a stop count taken from its
`interactive:` steps. Every builtin has one. A Graph without it, written as a
bare list of steps, still runs and still lists; Start offers it under
**More ›**.

`lf graph list --json` adds each Graph as a Task's workflow line draws it,
before any Task runs it:

```console
$ lf graph list --json | jq '.[] | select(.name=="code") | .workflow'
{
  "steps": [
    {"name": "plan", "interactive": false},
    {"name": "build", "interactive": false},
    {"name": "ship-review", "interactive": true},
    {"name": "deliver", "interactive": false}
  ],
  "interactive_steps": [...],
  "sentence": "You take part at ship-review."
}
```

`steps` are the top-level steps, with a Graph included by `graph:` as one
step under its name. A step is `interactive` when a person takes part anywhere
inside it, an `xor:` branch included. `interactive_steps` lists each of those
stops, and the stop count is its length. `sentence` is what Desktop's
[New Task](surfaces.md#new-task) sheet shows; a Graph with no stop reads "Runs
on its own." `workflow` is null for a Graph whose definition does not load.

### Auto

Auto is a setting on the Task: on, off, or never set. While it is on, the
Task's runs stand in for you at each interactive step they reach and may merge
the PR. Turning it on runs the Task: `lf task auto on` and Desktop's **Auto**
toggle are one operation.

```bash
lf task auto on LOO-12              # LOO-12 · code from ship-review running as Graph run …
lf task status LOO-12               # LOO-12  moving · auto: on by Maya, Nov 18 09:04
lf task auto off LOO-12             # LOO-12: auto off
```

- The run reads auto at each interactive step, not at launch: turn it on while
  `build` runs and that run stands in at `ship-review` when it gets there.
- `lf task auto off` kills nothing. The run waits at its next interactive step,
  or before a merging step (`lf land`, `lf pr arm`, `pr-land`): "Waiting at pr
  land: auto is off (Maya). `lf task auto on INF-1` lands it." Auto-merge that
  Loopflow armed is disarmed.
- Off stands: only `lf task auto on` turns it back on; an `auto: on` in
  configuration never does. `lf -b task run` stands in for its own run and
  writes no auto, so with auto off it still waits at the landing step.
- Never set, a run that can reach a merging step before stopping lands;
  `lf task run` prints ``merges: this run may merge the PR; `lf task auto off <ID>` stops it``
  (runs on non-merging Graphs such as `pursue` omit it). A failing run leaves
  auto alone.
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
| `idle` | Checked out, nothing running, not at an interactive step |
| `active` | Waiting at an interactive step in its Task conversation |
| `moving` | Graph running, whatever the auto setting |
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
