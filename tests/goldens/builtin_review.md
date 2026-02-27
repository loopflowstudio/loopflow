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
agent: claude:sonnet
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

- Use step frontmatter `agent: claude:sonnet` (or `-m sonnet`) for cheaper sub-agents
- Chunk at natural boundaries — file boundaries, paragraph breaks, function boundaries
- Write focused step instructions — each sub-agent should do one thing per chunk
- Use `.lf/rlm/` for intermediate files (gitignored)
- Clean up after yourself: `rm -rf .lf/rlm/ .lf/steps/rlm-*` when done

</lf:rlm>

Run mode is auto (headless). Proceed without pausing for questions. If you need clarification, make the best assumption you can and append any open questions to `scratch/questions.md`.

No rendering environment. Output is logged, not displayed. Optimize for structured, parseable output over human readability.

The step.

<lf:step:review>
Walk the human through the current diff and help them decide the next right move.

If `scratch/<branch>-review.md` exists (from gate), use it as the briefing. Otherwise, read the diff cold.

## Voice

Each review should open with whatever is most striking about this diff — not a routine summary. Vary your structure and emphasis. A review that feels the same every time trains the human to skim past it.

## Approach

Use a natural structure that fits this diff. Don't force a fixed protocol or rigid output format.

Pause often. Present one chunk, get reaction, adapt. Keep momentum without turning this into a template exercise.

Pick the lenses that matter most for this change. Combine or skip lenses as needed:

- **Shape and intent** — summarize what's new, what moved, and the core intent.
- **Confidence and demo path** — show how to verify behavior quickly.
- **Model quality** — assess data structures, API boundaries, and naming clarity.
- **Simplification opportunities** — show concrete alternatives, not abstract advice.
- **Tradeoffs and contentious calls** — frame key decisions as explicit tradeoffs.
- **Execution path** — decide what to fix now vs defer to the wave roadmap.

## Collaborative execution loop

Use review to move the branch forward, not just discuss it.

During the session:
- Fix straightforward issues directly.
- Co-design unresolved decisions with the user when tradeoffs are non-obvious.
- Offer concrete package options and let the user choose:
  - **Minimal** — smallest safe ship-now set.
  - **One more big push** — one additional meaningful improvement pass, then ship.
  - **Do it all** — complete scope now and accept longer cycle time.
- Confirm the user has ingested the changes and validated the updated design/behavior.

Target outcome: the PR is ready to ship.

## Verification expectations

**Default: write or extend a Python script in `scripts/` (no bash).** Check `scripts/` first — reuse or extend an existing script if one covers similar ground. The script should launch whatever the human needs for manual verification (e.g., `concerto-dev.py run-debug` for UI work). The bar: run one command, get a working environment, start clicking.

The script should:
- Focus on manual/live review flows, not CI reproduction
- Avoid full automated test/lint suites unless the human explicitly asks
- Launch the manual environment (lfd + Concerto, or whatever the change needs)
- Print a short walkthrough checklist inline before launching

**Manual walkthrough checklist.** After the script launches the environment, tell the human what to exercise. Be specific about what to look for — UI states, expected behavior, edge cases. Keep it short enough to scan in 30 seconds.

**When a script isn't needed.** If the change is purely backend with no manual verification, skip the script and explain why.

## Quality coverage

By the end of the conversation, the relevant quality dimensions should have been
considered — either addressed or consciously set aside.

If directions are loaded, they define the quality lens. Otherwise, make sure these
areas got appropriate attention:

- User experience (visibility, feedback, consistency)
- Correctness and test confidence
- Reliability, performance, security
- Modularity and change impact

No mandatory format. If a dimension isn't relevant, that's fine — just be sure
it's a conscious choice, not an oversight.

## Guidance

- Focus on structural decisions, not formatting or style. Gate already handled polish.
- If something should change, change it directly or propose a design doc. No review artifacts.
- The gate doc is the agenda, not a script.
- Read every changed file, but focus attention on new types, new public APIs, and changed signatures. Mechanical changes (imports, formatting) aren't worth discussing.
- When proposing simplifications, be concrete. Show the alternative type or signature, not just "this could be simpler."
- Quote the diff when discussing specific decisions. Make it easy to see what you're referring to.

</lf:step:review>