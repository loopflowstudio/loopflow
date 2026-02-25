# Wave Memory

## Problem

Every agent in a wave starts from zero. The codebase summary tells it what the code looks like, but not what the wave has learned — which tests need `--no-sandbox`, which patterns the user prefers, what failed last time. Agents repeat mistakes. Knowledge evaporates between runs.

Wave memory gives agents accumulated observations from prior runs. Not conversation logs — distilled knowledge. A wave that runs 20 times should be sharper than one running for the first time.

This is Phase 02 of the living wave, advancing these goals from `wave/living/README.md`:
- "Every wave-spawned agent starts with the wave's accumulated knowledge"
- "Agents write durable observations back to wave memory without special tools"
- "Memory consolidates naturally through existing steps"
- "The system prompt is the only injection point — no new APIs for memory"

## Approach

Add `DocumentSource::WaveMemory` to the context pipeline. Memory files live at `wave/<name>/memory/` and flow through prompt assembly like any other document source — gathered, budgeted, trimmed, formatted.

Three changes:

1. **Gather**: New `gather_wave_memory()` function reads `wave/<name>/memory/*.md` files. `SUMMARY.md` goes into `PromptComponents::summaries` (survives trimming longer). Topic files go into a new `PromptComponents::wave_memory` vec.

2. **Format**: New `<lf:memory>` section in `format_reference_sections()`, positioned after wave context and before docs. Tells the agent where memory lives and how to write back.

3. **Trim**: Wave memory topic files trim before docs but after area docs. `SUMMARY.md` inherits existing summary trimming behavior. Natural pressure: large memory → agents only see the summary → consolidation responds by compressing.

No new tools. No new APIs. No special write mechanism. Agents read memory because it's in the prompt. Agents write memory because the system prompt tells them to and they have `write_file` access.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Database-backed memory (SQLite/KV store) | Structured queries, dedup built in | Adds a dependency agents can't directly interact with. Plain files are readable, diffable, git-trackable. Over-engineering for markdown observations. |
| Append-only log with search | No risk of losing observations | Grows without bound. Agents would need a search tool. Consolidation becomes mandatory, not optional. Memory should be curated, not accumulated. |
| Memory as part of existing wave docs | No new DocumentSource needed | Blurs the line between planning (wave docs) and learned knowledge (memory). Different trimming priorities — memory should outlast area docs but not compete with the wave's own README. |
| Agent-local memory (per-agent files) | Each agent accumulates independently | Contradicts the design: agents are stateless, waves persist. Agent-local memory creates identity where the architecture explicitly avoids it. |

## Key decisions

**Memory is a new DocumentSource, not folded into Wave.** Wave docs (README, phases) are authored by humans and define intent. Memory is written by agents and captures experience. They need different trimming priorities: wave docs should survive longer than memory topic files when budget is tight.

**SUMMARY.md routes through `summaries`, topic files through `wave_memory`.** This reuses existing trimming behavior. Summaries drop after area docs but before main docs. Topic files drop even earlier (new trimming tier between area docs and summaries). When memory is too large, the summary survives and provides the essential context.

**The `<lf:memory>` prompt section includes write instructions.** Rather than a separate "memory awareness" bolt-on, the section that delivers memory content also tells the agent how to contribute. One section, both directions. This keeps the prompt focused and the mechanism obvious.

**Consolidation extends, not replaces.** The existing `consolidate` step gets a new section for memory review. It's the same step, same position in flows (end of `ship`), same agent. No new step.

**Memory is wave-private.** Cross-wave memory sharing (for chords/listening) is out of scope. The design doc raises it as an open question — keep it open. Wave-private memory is simpler and avoids the cross-wave contamination risk called out in `wave/living/README.md`.

**No bootstrap step.** New waves start with no memory. The first few runs build it organically. The existing summary mechanism handles cold starts. Adding a `seed-memory` step would front-load complexity for a problem that solves itself after 2-3 runs.

## Implementation

### 1. `DocumentSource::WaveMemory` variant

```rust
// prompt.rs
pub enum DocumentSource {
    Step,
    Direction,
    Wave,
    WaveMemory,  // new
    Area,
    Diff,
    Clipboard,
    RepoDoc,
    Summary,
}
```

### 2. `PromptComponents::wave_memory` field

```rust
pub struct PromptComponents {
    // ... existing fields ...
    /// Wave memory topic files (patterns.md, preferences.md, etc.)
    pub wave_memory: Vec<Document>,
}
```

### 3. `gather_wave_memory()` function

```rust
fn gather_wave_memory(repo_root: &Path, wave: Option<&str>) -> Result<(Vec<Document>, Vec<Document>), CoreError> {
    // Returns (summaries, topic_files)
    let Some(wave_name) = wave else { return Ok((vec![], vec![])) };
    let memory_dir = repo_root.join("wave").join(wave_name).join("memory");
    if !memory_dir.is_dir() { return Ok((vec![], vec![])) }

    let mut summaries = Vec::new();
    let mut topics = Vec::new();

    // SUMMARY.md → summaries (survives trimming longer)
    let summary = memory_dir.join("SUMMARY.md");
    if summary.is_file() {
        if let Ok(content) = fs::read_to_string(&summary) {
            summaries.push(Document {
                path: format!("wave/{}/memory/SUMMARY.md", wave_name),
                content,
                source: DocumentSource::WaveMemory,
            });
        }
    }

    // Other .md files → topic files (trim earlier)
    let mut entries: Vec<_> = fs::read_dir(&memory_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            path.is_file()
                && path.extension().map(|ext| ext == "md").unwrap_or(false)
                && path.file_name().map(|n| n != "SUMMARY.md").unwrap_or(false)
        })
        .collect();
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
        if let Ok(content) = fs::read_to_string(&path) {
            topics.push(Document {
                path: format!("wave/{}/memory/{}", wave_name, path.file_name().unwrap_or_default().to_string_lossy()),
                content,
                source: DocumentSource::WaveMemory,
            });
        }
    }

    Ok((summaries, topics))
}
```

Called from `gather_documents()` after wave docs:

```rust
if spec.includes(DocumentSource::WaveMemory) {
    let (mem_summaries, mem_topics) = gather_wave_memory(&spec.repo_root, spec.wave.as_deref())?;
    // summaries and topics returned separately for different trimming
}
```

### 4. Trimming order update

Current: area docs → summaries → docs → diff → clipboard

New: area docs → **wave memory topics** → summaries → docs → diff → clipboard

```rust
// In trim_context_with_breakdown(), after area docs trimming:

// 1.5. Drop wave memory topic files (learned observations, regenerable via consolidation)
while total > max_tokens && !components.wave_memory.is_empty() {
    components.wave_memory.pop();
    if let Some(tokens) = wave_memory_tokens.pop() {
        breakdown.subtract_source_tokens(DocumentSource::WaveMemory, tokens);
        total = total.saturating_sub(tokens);
    }
}
```

Memory SUMMARY.md routes through `summaries` and gets the same treatment as codebase summaries.

### 5. Prompt formatting

New section in `format_reference_sections()`, after wave context (line ~1232) and before docs:

```rust
// Wave memory
if !components.wave_memory.is_empty() || components.summaries.iter().any(|s| s.source == DocumentSource::WaveMemory) {
    let wave_name = components.wave.as_deref().unwrap_or("unknown");
    let mut memory_parts = Vec::new();

    // Include memory SUMMARY.md from summaries
    for s in &components.summaries {
        if s.source == DocumentSource::WaveMemory {
            memory_parts.push(format!("<lf:memory-summary>\n{}\n</lf:memory-summary>", s.content));
        }
    }

    // Include topic files
    for doc in &components.wave_memory {
        let name = Path::new(&doc.path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| doc.path.clone());
        memory_parts.push(format!("<lf:{}>\n{}\n</lf:{}>", name, doc.content, name));
    }

    let memory_body = memory_parts.join("\n\n");
    parts.push(format!(
        "This wave has persistent memory at wave/{wave_name}/memory/.\n\
         Write observations that should persist for future agents:\n\
         - Codebase patterns and conventions you discover\n\
         - User preferences you observe\n\
         - Things that worked or failed\n\
         - Corrections to existing memory\n\n\
         Don't write session-specific notes. Only durable observations.\n\
         Update existing files rather than creating new ones when possible.\n\n\
         <lf:memory>\n{memory_body}\n</lf:memory>"
    ));
}
```

### 6. Consolidate step extension

Add to the existing `consolidate.md` prompt:

```markdown
## Wave memory (if wave-scoped)

If this run is wave-scoped, review `wave/{wave}/memory/` for:
- Duplicate or redundant observations — merge them
- Stale information contradicted by current code — remove it
- SUMMARY.md accuracy — regenerate if needed
- Overall size — compress if memory exceeds ~2000 tokens total
```

### 7. `gather_wave_docs()` exclusion

Update `gather_wave_docs()` to skip the `memory/` subdirectory — memory files have their own gathering path and DocumentSource:

```rust
fn gather_wave_docs(repo_root: &Path, wave: Option<&str>) -> Result<Vec<Document>, CoreError> {
    // ... existing code ...
    // Skip memory/ directory — handled by gather_wave_memory()
    let mut entries: Vec<_> = fs::read_dir(&wave_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            path.is_file()  // already excludes directories
                && path.extension().map(|ext| ext == "md").unwrap_or(false)
                && path.file_name().map(|n| n != "README.md").unwrap_or(false)
        })
        .collect();
```

Note: `gather_wave_docs()` already filters to `is_file()`, so the `memory/` directory is already excluded. No change needed here. But if the function ever changes to recurse into subdirectories, memory files would be double-counted. Worth a comment.

## Scope

**In scope:**
- `DocumentSource::WaveMemory` variant
- `PromptComponents::wave_memory` field
- `gather_wave_memory()` function
- Trimming tier for wave memory topics
- `<lf:memory>` prompt section with write instructions
- Consolidate step memory awareness
- Tests: golden prompt test with memory files present, trimming test verifying drop order

**Out of scope:**
- Cross-wave memory sharing (open question, keep it open)
- Memory inheritance on split-wave (Phase 04 — memory lifecycle)
- Bootstrap/seed-memory step (organic growth is sufficient)
- Memory aging (Phase 04)
- CLI memory visibility (`lf implement -w engbot` seeing memory — trivial to add later since it flows through the same `gather_context` path, but not blocking)
- Agent identity or conversation persistence (explicitly not here per wave vision)

## Done when

1. `cargo test --all` passes with new `WaveMemory` variant wired through gather → trim → format
2. Golden prompt test: a wave run with `wave/<name>/memory/{SUMMARY.md, patterns.md}` produces a prompt containing `<lf:memory>` with both files' content and write instructions
3. Trimming test: wave memory topics drop before summaries, memory SUMMARY.md drops with summaries
4. Manual verification: run `lf implement -w living` with memory files present, confirm agent sees memory in prompt log (`.lf/log/*-implement.context`)
5. Consolidate step extended (prompt update only — no Rust changes needed)
