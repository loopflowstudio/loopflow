<lf:rlm>
# RLM: Recursive Language Model

Spawn sub-agents to process inputs beyond your context window. Split, delegate, aggregate.

---

## When to Use

When exhaustive analysis requires more data than fits in context:
- Files too large to read at once
- Many files across a broad area
- Logs, transcripts, or documents requiring complete coverage

Don't use RLM for tasks where reading a few key files is sufficient. Use it when partial reading would miss things.

---

## Pattern

Every RLM task follows the same four steps regardless of environment.

```
You (parent agent)
├── 1. Examine: check size, decide whether to split
├── 2. Split: divide into chunks at natural boundaries
├── 3. Delegate: spawn one sub-agent per chunk
│   ├── Sub-agent 1 → chunk A → result A
│   ├── Sub-agent 2 → chunk B → result B
│   └── Sub-agent 3 → chunk C → result C
└── 4. Aggregate: combine results, answer original question
```

Sub-agents can themselves recurse — if a chunk is still too large, the sub-agent splits further.

### 1. Examine

Check size and structure before deciding to split.

```bash
wc -l large_file.txt
find src/api/ -name "*.rs" | wc -l
```

If it fits in context, just read it. RLM adds overhead — only use it when you'd miss things otherwise.

### 2. Split

Divide at natural boundaries — file boundaries, function/class boundaries, paragraph breaks, or fixed line counts for uniform data.

### 3. Delegate

Spawn one sub-agent per chunk. Use whatever mechanism your environment provides.

#### Agent tool (Claude Code)

If you have access to an Agent tool, use it directly. Embed the chunk in the prompt. Launch multiple sub-agents in parallel by making multiple Agent calls in a single response.

Use a cheap, fast model (`haiku`) for chunk processing:

```
Agent tool call:
  description: "Scan chunk 1 for endpoints"
  subagent_type: "general-purpose"
  model: "haiku"
  prompt: |
    Find all public API endpoints in this code.
    Return each as: METHOD /path — description

    <code>
    [chunk content]
    </code>
```

Launch all chunks in parallel — independent Agent calls in the same message execute concurrently. Each sub-agent gets an isolated context window.

To aggregate, read the returned results from each Agent call and synthesize.

#### Shell — `lf -b` (Codex, OpenCode, Gemini, terminal)

If you don't have an Agent tool, use shell to write chunks to disk and run `lf` in batch mode.

```bash
mkdir -p .lf/rlm/chunks .lf/rlm/results .lf/steps

# Split the input
split -l 500 large_file.txt .lf/rlm/chunks/chunk_

# Create a step per chunk
for f in .lf/rlm/chunks/chunk_*; do
  name=$(basename "$f")
  cat > ".lf/steps/rlm-${name}.md" <<EOF
---
agent: claude:haiku
---
Find all public API endpoints in this code.
Write each as METHOD /path to .lf/rlm/results/${name}.out

<code>
$(cat "$f")
</code>
EOF
done

# Run in parallel (up to 10 concurrent)
ls .lf/steps/rlm-*.md | xargs -P 10 -I {} sh -c \
  'lf "$(basename {} .md)" -b'
```

To aggregate:

```bash
cat .lf/rlm/results/*.out | sort -u > .lf/rlm/combined.txt
```

Then read the combined file and answer the original question.

Sub-agents spawned via `lf -b` get full loopflow context (repo docs, style guide, area docs). Recursion depth increments automatically.

### 4. Aggregate

Combine sub-agent results into a final answer. Look for:
- Duplicates across chunks (deduplicate)
- Items that span chunk boundaries (reconcile)
- Patterns only visible across the full set (synthesize)

---

## Recursion Depth

Depth prevents infinite recursion. Each nested `lf` invocation auto-increments `RLM_DEPTH`. Check before spawning sub-agents that might themselves recurse.

| Variable | Default | Purpose |
|----------|---------|---------|
| `RLM_DEPTH` | 0 | Current recursion depth (auto-incremented by `lf`) |
| `RLM_MAX_DEPTH` | 3 | Maximum allowed depth |
| `RLM_MAX_PARALLEL` | 10 | Suggested max concurrent sub-agents |

When using the Agent tool, environment variables don't propagate — track depth yourself. If you've already split once, tell sub-agents not to recurse further unless the chunk is genuinely too large.

---

## Tips

- **Cheap models for sub-agents.** Use `haiku` or `sonnet` — chunk processing is mechanical, not creative.
- **Chunk at natural boundaries.** File, function, paragraph. Don't split mid-sentence or mid-function.
- **Focused instructions.** "Find X in this code" not "analyze this code." Each sub-agent does one thing.
- **Don't over-split.** 5–20 chunks is typical. 100 chunks means your task needs a different approach.
- **Clean up.** `rm -rf .lf/rlm/ .lf/steps/rlm-*` when done.

</lf:rlm>

Run mode is headless. No user is present. Never ask questions or wait for input — no one will answer.

Do the work. Make executive decisions where needed — pick the simpler choice and keep moving. You can always be corrected in review.

If something is genuinely ambiguous, note your assumption in `scratch/questions.md` and proceed with your best judgment. Do not stop.

No rendering environment. Output is logged, not displayed.

The step.

<lf:step:code-review>
Walk through structural and architectural decisions with the human. The diff is the starting point; the codebase's trajectory is the subject.

## Voice

The human chose to look at code, not behavior. They're thinking about architecture — how this change fits into the larger vision. Meet them there. Don't narrate the diff mechanically; orient them in the design space this change opens up.

Vary structure based on what matters here. A refactor that simplifies a module needs different treatment than one that introduces a new pattern across the codebase.

## Opening

Orient the human in the architectural context:

1. **What changed structurally** — what moved, what was introduced, what was removed. In terms of types, boundaries, and relationships — not files.
2. **The design intent** — why this shape, as best you can read it from the diff and any scratch docs. State it plainly so the human can confirm or correct.
3. **Where this sits** — how the changed code relates to its surroundings. What depends on it, what it depends on.

## Approach

The conversation moves outward from the diff into the broader architecture. Don't stay zoomed in on what changed — the human is here to think about trajectory.

Pick the lenses that matter:

- **Pattern integration** — does this change introduce or reinforce patterns that the surrounding code should adopt? Or does it create a second way of doing things?
- **Architectural direction** — does this pull the codebase toward where it wants to go? What would the natural next step look like after this lands?
- **Simplification** — did this change reveal unnecessary complexity nearby? Show concrete alternatives.
- **Boundaries and seams** — are the module boundaries in the right place? Would moving a boundary make multiple things simpler?
- **Consistency** — does the surrounding code want to be updated to match, or does this change want to match the surrounding code?

Pause often. Present one observation, get the human's take. Their sense of where the architecture should go is primary.

## Beyond the diff

This is what makes code-review different from a standard diff walkthrough. Actively look at surrounding code — not just what changed, but what's adjacent:

- Code that does similar things differently than this change
- Patterns in the area that this change could extend or that could adopt this change's approach
- Structural decisions in nearby modules that interact with these changes

Present what you find. "The change introduces X pattern here. Three files nearby still do it the old way — is that the next step, or should this match them instead?"

## Collaborative execution

During the session:
- Fix clear wins directly. Naming improvements, dead code removal, consistency fixes — just do them.
- Co-design when the trajectory question has real tradeoffs. "We could push this pattern through the whole module now, or let it prove itself here first."
- Packaging options when scope expands:
  - **Ship as-is** — the change is sound, surrounding code can evolve later.
  - **Extend** — push the pattern/improvement into adjacent code while context is fresh.
  - **Redesign** — this change revealed something bigger; go back to design.

## Guidance

- Focus on structure and decisions, not formatting or style. Linters handle style.
- When proposing alternatives, sketch them. Show the type, the signature, the relationship — not just "this could be simpler."
- Quote the diff when discussing specific decisions.
- Read surrounding code, not just changed files. The architectural context matters more than the diff in isolation.
- If directions are loaded, use them as the quality lens. Otherwise, consider modularity, clarity, and whether the change compounds well.

## Adaptation

When architectural patterns or conventions emerge that aren't documented, add them to repo docs (CLAUDE.md, STYLE.md) so all steps benefit.

</lf:step:code-review>
