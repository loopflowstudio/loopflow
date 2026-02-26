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
