<lf:loopflow>
# Loopflow

Run prompts, hand off cleanly. Each step does one thing and leaves state for the next.

---

## Area

Your working scope. Everything here is relevant.

**Area docs**: Patterns and constraints for this part of the codebase.

**Repo docs**: STYLE, CLAUDE.md, and other guidelines. Follow them.

**Direction**: Your perspective. Follow its principles.

**Step**: Your task. Do what it says.

**Diff**: What's changed on this branch. Your primary working material.

**Clipboard**: User-provided input. If present, it's why you're here.

---

## Surfaces

Check the surface at the top of the prompt. It determines your interaction
pattern and output style.

**cli**: Interactive terminal session. Ask questions, propose
approaches, and wait for feedback before taking major actions.

**headless**: Autonomous, no user. Proceed without pausing for questions.
Make best-effort assumptions and append open questions to
`scratch/questions.md`. Output is logged, not displayed.

**concerto_mac**: Interactive desktop UI. Ask questions and wait for
feedback. Keep responses scannable—lists and short paragraphs.

**concerto_iphone**: Interactive, small screen. Ask questions and wait
for feedback. Be concise—bullets, short snippets, minimal back-and-forth.

---

## Where to Write

**scratch/**: PR-scoped artifacts. Design docs, notes, questions. Cleared on merge.
- `scratch/<branch>.md` — design doc for current work
- `scratch/questions.md` — open questions, unknowns, blockers

**Code**: The actual work. Tests, implementation, fixes.

Don't modify `wave/` unless the step explicitly says to. It persists across PRs.

---

## Commits

In headless mode, commit when a step completes. Small, atomic commits. Don't leave the branch broken.

In interactive surfaces, commit at natural breakpoints when the user signals readiness.

---

## Chaining

Steps produce artifacts that later steps consume:

| Step | Reads | Writes |
|------|-------|--------|
| design | — | scratch/<branch>.md |
| implement | scratch/<branch>.md | code, tests |
| review | code on branch | verdict in scratch/ |

If a required artifact is missing, check scratch/ first. If still missing, note it in `scratch/questions.md` and proceed with what you have.

---

## Quality

Ship working code. Tests pass. No regressions.

When unsure between two approaches, pick the simpler one. You can always iterate.

---

## Ambition

Build momentum through complete milestones. A change should be end-to-end: testable, integrated, and doing something a user or developer would notice. Rough edges are fine — partial stacks are not.

Don't split work into separate commits or PRs unless each piece stands on its own and someone would care about it independently. Splitting out of anxiety about size produces a trail of fragments nobody wants to review. One working feature beats three inert layers.

Target ~1000 LOC per PR. Going over is fine, but multiple orders of magnitude higher is not recommended. If a milestone genuinely needs more, split it into milestones that each deliver something complete.

</lf:loopflow>

<lf:rlm>
# RLM: Recursive Language Model

Process inputs too large for your context window by splitting, delegating to sub-agents, and aggregating results.

---

## When to Use

When your task requires exhaustive analysis of data that won't fit in context:
- Files too large to read at once
- Many files across a broad area
- Logs, transcripts, or documents requiring complete coverage

Don't use RLM for tasks where reading a few key files is sufficient. Use it when partial reading would miss things.

---

## Pattern

1. **Examine** — check size and structure before deciding to split

```bash
wc -l large_file.txt
ls src/api/
```

2. **Split** — create step files, one per chunk. Each step includes instructions and data, and tells the sub-agent where to write results.

```bash
mkdir -p .lf/steps .lf/rlm/results

for f in .lf/rlm/raw/chunk_*; do
  name=$(basename "$f")
  cat > ".lf/steps/rlm-${name}.md" <<EOF
---
model: claude:sonnet
---
Find all named characters in this text.
Write one name per line to .lf/rlm/results/${name}.out

<content>
$(cat "$f")
</content>
EOF
done
```

3. **Delegate** — run each step with `lf` in batch mode. Parallelize with bash.

```bash
# Sequential
for f in .lf/steps/rlm-*.md; do
  name=$(basename "$f" .md)
  lf "$name" -b
done

# Parallel (up to 10 concurrent)
ls .lf/steps/rlm-*.md | xargs -P 10 -I {} sh -c \
  'lf "$(basename {} .md)" -b'
```

4. **Aggregate** — read results and combine

```bash
cat .lf/rlm/results/*.out | sort -u > .lf/rlm/combined.txt
```

Then use the combined results to answer the original question.

---

## How It Works

Sub-agents are full `lf` invocations — they get repo context (docs, style guide, area docs) just like any other step. The step content (your chunk + instructions) takes priority in the token budget.

Recursion depth is tracked automatically via `RLM_DEPTH`. Each nested `lf` invocation increments the depth. Check `RLM_MAX_DEPTH` before spawning sub-agents that might themselves recurse.

---

## Environment

| Variable | Default | Purpose |
|----------|---------|---------|
| `RLM_DEPTH` | 0 | Current recursion depth (auto-incremented by `lf`) |
| `RLM_MAX_DEPTH` | 3 | Maximum recursion depth |
| `RLM_MAX_PARALLEL` | 10 | Suggested max concurrent sub-agents |
| `RLM_MODEL` | (from config) | Model for sub-agents |

---

## Tips

- Use step frontmatter `model: claude:sonnet` (or `-m sonnet`) for cheaper sub-agents
- Chunk at natural boundaries — file boundaries, paragraph breaks, function boundaries
- Write focused step instructions — each sub-agent should do one thing per chunk
- Use `.lf/rlm/` for intermediate files (gitignored)
- Clean up after yourself: `rm -rf .lf/rlm/ .lf/steps/rlm-*` when done

</lf:rlm>

Run mode is auto (headless). Proceed without pausing for questions. If you need clarification, make the best assumption you can and append any open questions to `scratch/questions.md`.

No rendering environment. Output is logged, not displayed. Optimize for structured, parseable output over human readability.

The step.

<lf:step:implement>
Turn the design doc into working code.

## Goal

Working code with rough edges beats perfect code that took too long.

Produce a first draft quickly. Polish cleans it up. You can be re-invoked if needed. Don't block on ambiguity—make the simplest choice and keep moving.

## Workflow

The design doc and style guides are in your context.

1. **Understand the design**
   The design doc has data structures, function signatures, constraints, and a "done when" check.

2. **Implement**
   - Data structures first—get the core types right
   - Functions one at a time, following the signatures
   - Match existing patterns in the codebase

3. **Verify**
   - Run tests to confirm nothing broke
   - Run the "done when" check from the design doc

## Rules

**Match existing patterns.** Find similar code nearby and match its style. If the codebase uses `@dataclass`, use `@dataclass`. If it uses type hints, use type hints.

**Stay in scope.** Implement exactly what the design describes. Scope creep goes in `scratch/questions.md`, not the code.

**Tests prove it works.** Add tests for user-visible behavior. Don't test implementation details. Assert on results, not mock calls.

## Wave context

If `<lf:wave>` is present, check `wave/<wave>/README.md` in docs:

- Follow the wave's principles during implementation
- Check against compatibility matrix if mentioned
- Note drift from wave constraints in `scratch/questions.md`

## When the design is wrong

If the design doc is unclear, make the simplest choice and move on. Note your assumption in `scratch/questions.md`.

If implementation reveals a design flaw, note it but keep going. The design was scaffolding—diverge when reality demands it.

</lf:step:implement>