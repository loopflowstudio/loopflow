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

<lf:wave name="rust">
You are building toward the rust program of work.
Wave context is included in docs below.

## Wave memory

Persistent memory at wave/rust/MEMORY.md. Budget: ~25k tokens.
Read it before you start. Update it aggressively — correct stale entries,
add observations, remove what's wrong. Don't wait until the end of your session.

Suggested sections — Patterns, Preferences, Learnings — but add your own as needed.
- Patterns: codebase conventions, architecture, how things connect
- Preferences: user workflow, tool choices, communication norms
- Learnings: what worked, what failed, surprises

What belongs elsewhere:
- architectural decisions → wave docs or area docs
- design rationale → scratch/ or wave plan
- session-specific notes → nowhere (let them die)

How to update:
- Edit within sections. Don't rewrite the whole file.
- Correct or remove entries that are wrong or stale.
- Use absolute dates, not "today" or "recently".
- When a section grows large, promote stable entries to wave/area docs and trim.

<lf:memory path="wave/rust/MEMORY.md">
- Keep prompts concise and concrete.
- Prefer behavior-focused tests over mock wiring.

</lf:memory>
</lf:wave>

Repository documentation. Follow STYLE carefully. May include design artifacts (scratch/).

<lf:docs>
<lf:design>
# Design

Current design notes.

</lf:design>

<lf:README>
# Rust Roadmap

Overview of Rust work.

</lf:README>

<lf:README>
# Test Repo

Root readme.

</lf:README>
</lf:docs>

The step.

<lf:step:test>
# Test step

Do the thing.

</lf:step:test>