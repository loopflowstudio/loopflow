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
