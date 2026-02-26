# Review: Route wave name into commit message generation

## What was implemented

Wave-aware commit messages. When a wave executor auto-commits, the wave name is now used as the commit message area prefix (e.g. `engbot: add retry logic` instead of `llm_http: add retry logic`). This gives wave-driven work a consistent, traceable commit prefix without the LLM guessing a topic.

Three parts:
1. `CommitOptions` gains a `wave: Option<String>` field and a `for_task()` constructor
2. `generate_commit_message()` appends a `## Topic` section to the LLM prompt when a wave name is present
3. Builtin step/ops prompts updated to document wave-name-as-prefix convention

## Key choices

**Dynamic prompt injection over static prompt variants.** The `COMMIT_MESSAGE_PROMPT` constant stays generic. When `wave` is `Some`, a `## Topic` section is appended that overrides the area prefix. This keeps the base prompt simple and avoids a second constant.

**`CommitOptions::for_task()` constructor.** Seven callsites were constructing `CommitOptions` with mostly-default fields. The constructor eliminates boilerplate and ensures new fields (like `wave`) get sensible defaults everywhere without touching every callsite. Uses struct update syntax (`..CommitOptions::for_task(...)`) so callsites only specify what they override.

**Wave name passed as `Option<String>`, not a context object.** Keeps the ops layer decoupled from wave/executor types. Only the wave executor knows the wave name; everything downstream just sees an optional string.

## How it fits together

```
WaveExecutor (knows wave name)
  → auto_create_pr(worktree, Some(wave_name))
    → CommitOptions { wave: Some(name), .. }
      → commit_workflow()
        → generate_commit_message(repo, wave.as_deref())
          → appends "## Topic\nAlways use `{name}` as the area prefix"
```

The builtin `.md` prompts (`commit_message.md`, `commit.md`) handle the same concern via `<lf:wave>` context tags during prompt assembly — a parallel path for `lf commit` (interactive step) vs `lf ops commit` (programmatic).

## Risks and bottlenecks

- **LLM compliance.** The `## Topic` injection tells the LLM "always use X as prefix, do not invent a different topic." If the LLM ignores it, commits get inconsistent prefixes. The existing `Message::validate()` checks title length and URLs but not prefix correctness. Acceptable risk — the instruction is strong and direct.
- **No wave propagation to PR messages.** `generate_pr_message()` doesn't receive the wave name. PR titles may still get LLM-chosen prefixes. This is likely intentional scope — PR titles summarize the whole branch, not individual commits.

## What's not included

- Wave name in PR message generation (only commit messages)
- Validation that commit titles actually start with the wave name
- Migration of existing commit history
