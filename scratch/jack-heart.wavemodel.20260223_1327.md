# Wave Content Model — Commit 1

Define the standard wave README shape, migrate existing READMEs, and update step prompts to reference it.

## What to build

Every wave README gets a standard content model: **Vision, Goals, Risks, Metrics, Roadmap** — at least one required. Steps learn to read and maintain these sections by name.

> "flow, area, and direction are configuration. These fields are content." — user

> "I'm imagining we maintain a README.md that contains those sections, and our wave steps are written to maintain them." — user

## The README shape

```
wave/<name>/
├── <name>.yaml    # configuration: flow, area, direction, stimulus
├── README.md      # content: vision, goals, risks, metrics, roadmap
├── 01-phase.md    # roadmap items (existing pattern)
└── 02-phase.md
```

Two layers, fully separate. YAML is what the engine reads. README is what agents and humans read.

### Standard sections

A wave README has AT LEAST ONE of these five sections:

```markdown
# Wave Name

One-line summary.

## Vision

Why this wave exists. The north star.
What the world looks like when this wave is done.
What's explicitly not here (scope boundaries).

## Goals

Concrete objectives. What "done" looks like.
Invariants that must hold throughout.
Success criteria a reviewer can check.

## Risks

What could go wrong. What we're watching for.
Open questions and unknowns.
Threat models where relevant.
Things that might change and why.

## Metrics

How we know it's working. Observable signals.
Not just "tests pass" — evidence the wave is achieving its vision.
Commands to run, outputs to expect, behaviors to observe.

## Roadmap

Staged plan. Phases with status, sequencing, dependencies.
Post-ship adjustments tracked inline.
```

Supplementary sections (Architecture, Design Decisions, API, Core Components) remain valid alongside the five. The standard five are the strategic spine; supplementary sections are tactical detail.

## Migration mapping

| Existing pattern | Standard section |
|-----------------|-----------------|
| North Star | Vision |
| What's not here | Vision (scope) |
| Invariants | Goals |
| Done when | Goals |
| Security boundary | Goals |
| Threat Model | Risks |
| What might change | Risks |
| Open questions | Risks |
| Phases | Roadmap |
| Post-ship adjustments | Roadmap (inline) |
| Design Decisions | (keep alongside) |
| Architecture | (keep alongside) |
| Core components | (keep alongside) |

### Per-wave migration notes

**agentapi** — "North Star" → Vision. "Invariants" → Goals. "Phases" → Roadmap. "What's not here" → fold into Vision scope. "Design Decisions" and "Architecture" and "API" stay as supplementary.

**security** — "North Star" → Vision. "Security boundary" + "Security invariants" → Goals. "Threat Model" + "What might change" → Risks. "Phases" + "Post-ship adjustments" + "Current scope cut" → Roadmap. "Reference Frameworks" stays as supplementary. "What's not here" → fold into Vision scope.

**loop** — "North Star" → Vision. "Locked v1 decisions" + "Done when" → Goals. "What might change" + "Plan adjustments" → Risks. "Remaining phases" + "Status after shipping" → Roadmap.

**remote** — "North Star" → Vision. "What's not here" → Vision scope. "Update after Phase 01D" open questions → Risks. "Phases" → Roadmap. "Design Decisions" and "Architecture" stay as supplementary.

**harness** — Opening paragraph + "Core components" → Vision. "Invariants" + completion criteria from B3 → Goals. "What might change" + "Open questions" from B3 → Risks. "Two tracks" (A/B phases) → Roadmap. "Design decisions" and "Core components" stay as supplementary.

All five get a new **Metrics** section. Most will be thin initially — observable signals like "tests pass", "API works end-to-end", "no regressions in existing behavior." Better than absent.

## Step prompt updates

Six steps become section-aware. Changes are surgical additions, not rewrites.

### `update-wave` (ops/update-wave.md)

Current "What to update" lists ad-hoc sections. Replace with:

```markdown
## What to update

- **Roadmap** — update phase status, revise scope based on what we learned
- **Risks** — add new risks discovered during implementation, resolve answered questions
- **Goals** — refine success criteria if they evolved, update invariants if new ones emerged
- **Metrics** — note any observable signals from what we shipped
- **Vision** — should rarely change. If it does, flag it explicitly — vision drift is a design decision, not a side effect
```

Keep the existing guidance about not rewriting distant phases and not expanding scope.

### `ingest` (plan/ingest.md)

Add to "Using README.md" section:

```markdown
**Using README.md:**
- Read **Vision** to understand what the wave is trying to achieve
- Read **Goals** to evaluate priority — what moves success criteria most?
- Read **Risks** to evaluate urgency — is something blocked or at risk?
- Read **Roadmap** to understand sequencing and dependencies
- Respect scope boundaries stated in Vision — don't pick items that conflict
```

### `kickoff` (plan/kickoff.md)

Update "Wave alignment" section:

```markdown
## Wave alignment

If `<lf:wave>` is present, check the wave README:

- **Vision** — design must serve the wave's north star
- **Goals** — "Done when" must contribute to wave success criteria. Quote the specific goals you're advancing.
- **Risks** — "Imagine wild failure" should check against known risks. If this design introduces a new risk, name it.
- Scope must exclude what Vision marks as "not here"
```

### `gate` (code/gate.md)

Add to Phase 2 (Polish Docs), after "Update README and docs":

```markdown
4. **Wave alignment** (if running in a wave context)
   - Does the shipped code advance the wave's Goals?
   - Were any known Risks from the wave README introduced or ignored?
   - Are there observable Metrics to note in the review doc?
```

### `review` (interactive/review.md)

Add to existing phases:

- Phase 3 (Core model): "If this wave has a Vision, does the model serve it?"
- Phase 5 (Contentious calls): "Check against the wave's Goals and Risks — do any decisions conflict?"
- Phase 6 (Learnings): "Should the wave's Risks, Goals, or Metrics be updated based on what we learned?"

### `design` (interactive/design.md)

Add wave content model awareness to Phase 4 (Fork). When writing `scratch/wave-proposal.md`:

```markdown
**If wave plan:**

1. Break the idea into staged wave items
2. Write `scratch/wave-proposal.md` using the wave content model:
   - `## Vision` — from the Dream phase conversation
   - `## Goals` — concrete objectives from the Detail phase
   - `## Risks` — unknowns and failure modes surfaced during detailing
   - `## Metrics` — observable signals discussed
   - `## Roadmap` — the staged breakdown
3. The first stage becomes the design doc for this branch (`scratch/<branch>.md`)
```

Full wave-directory creation (writing to `wave/<name>/` directly) comes in the next commit.

## Constraints

- No Rust or Python code changes. All markdown/prompt content.
- Existing wave README content preserved — restructured, not rewritten.
- Steps remain backwards-compatible: a README missing sections still works.
- YAML schema unchanged. Configuration and content are separate layers.

## Done when

1. `grep -l "## Vision\|## Goals\|## Risks\|## Metrics\|## Roadmap" wave/*/README.md` returns all 5 wave READMEs
2. All 6 step prompts reference standard sections by name
3. No content lost from existing READMEs — verify by diffing before/after
