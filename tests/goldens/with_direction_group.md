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

## Run Modes

Check the run mode at the top of the prompt.

**If auto mode**: Run to completion. Don't pause for questions. Make best-effort assumptions. Write open questions to `scratch/questions.md` and keep moving.

**If interactive mode**: Ask clarifying questions when needed. The user will guide you.

---

## Where to Write

**scratch/**: PR-scoped artifacts. Design docs, notes, questions. Cleared on merge.
- `scratch/<branch>.md` — design doc for current work
- `scratch/questions.md` — open questions, unknowns, blockers

**Code**: The actual work. Tests, implementation, fixes.

Don't modify `wave/` unless the step explicitly says to. It persists across PRs.

---

## Commits

In auto mode, commit when a step completes. Small, atomic commits. Don't leave the branch broken.

In interactive mode, commit at natural breakpoints when the user signals readiness.

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

The step.

<lf:step:test>
Test step content with builtin direction group.

</lf:step:test>

Directions for this work.

<lf:directions>
<lf:direction:care>
Quality and attention to detail. Take time to get it right. No shortcuts.

What would this look like if we had infinite time? Now do 80% of that.

- Edge cases handled, not ignored
- Error messages a user will actually read
- Naming that teaches — someone unfamiliar learns the domain by reading the code
- Consistency that compounds — small decisions aligned across the codebase
- Refactor when needed, not when convenient

</lf:direction:care>
<lf:direction:clarity>
Design around data structures and public APIs. 1:1 mapping between real-world concepts and code.

Code demonstrates its own correctness. If a feature exists, a test proves it works.

- Name things after what they are: Document, FileEdit, Target — not DocumentHelper, EditResult, OutputHandler
- Aim for a reader to understand the system by reading the types and their relationships
- Make it easy to see what's done and what's broken
- One source of truth per concept

</lf:direction:clarity>
<lf:direction:scale>
Build for growth. Prefer horizontal scaling, stateless design, async patterns.

Avoid premature optimization but design for 10x current load.

- Caching, sharding, queues, idempotency — reach for these before inventing
- Stateless where possible; explicit state where necessary
- Design for 10x, not 100x — you'll rewrite before you get there
- Measure first, scale second

</lf:direction:scale>
<lf:direction:simplicity>
Every line of code earns its place. Readable, not terse — but recognize that lines can be net-negative.

Start with minimal data structures and APIs. If the core is right, trimming excess is straightforward.

- Unused code, obvious comments, impossible-condition checks — all net-negative
- Don't add features, refactor code, or make improvements beyond what was asked
- Three similar lines of code is better than a premature abstraction
- When in doubt between two approaches, pick the simpler one

</lf:direction:simplicity>
</lf:directions>