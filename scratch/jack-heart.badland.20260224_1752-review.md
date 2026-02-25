# Review: Harden PR message generation

## What was implemented

Hardened `generate_message()` in `messages.rs` to prevent garbage LLM output from reaching irreversible operations (PR titles, squash merge commits). Three changes:

1. **Strict JSON parsing** — Removed the fallback that treated any first-line-as-title when JSON parsing failed. `parse_message_output` now returns `Err` if no valid JSON is found.

2. **Message validation** — `Message::validate()` rejects titles containing URLs, titles over 100 chars, and empty bodies when the diff is non-trivial (>=20 changed lines). This catches the exact failure pattern from PR #396.

3. **Output logging** — Raw agent stdout/stderr is written to `.lf/logs/` before parsing, so failures can be diagnosed after the fact. Log path is appended to error messages as a hint.

Supporting changes: `MessageKind` enum distinguishes commit vs PR messages (commits don't require a body), duplicate `generate_pr_message` / `generate_pr_message_from_diff` paths collapsed into `generate_pr_message_with_diff`.

## Key choices

**Fail hard on non-JSON output.** The old fallback parser accepted anything — a status message, a URL, even a stack trace — and silently used it as the PR title. The new parser requires JSON with `title` and `body` keys. This is more strict but the prompt already asks for JSON, and the three extraction strategies (raw JSON, fenced code block, inline braces) cover all reasonable agent output formats.

**Validation at the `Message` level, not in the parser.** Parsing produces a `Message`; validation checks its content. This separation lets callers validate with different rules (commit messages don't need bodies) and keeps test surface clean.

**Best-effort logging.** `write_message_output_log` returns `Option<PathBuf>` and silently skips if the directory can't be created. Message generation shouldn't fail because logging failed.

**20-line threshold for "non-trivial diff".** Arbitrary but reasonable — anything under 20 changed lines is a small fix where a title-only description suffices.

## How it fits together

```
generate_pr_message()
  → generate_pr_message_with_diff()    # computes non_trivial_diff
    → generate_message()               # launches agent, logs output
      → parse_message_output()         # strict JSON extraction
      → Message::validate()            # content checks
      → append_log_hint_to_error()     # enriches errors with log path
```

`finalize_remote()` in `land.rs` calls `generate_pr_message()` and gets back either a validated `Message` or an error with a log path hint. The error stops the land flow before the title reaches GitHub.

## Risks and bottlenecks

- **Agent output format changes.** If a future agent model stops wrapping output in JSON (or uses a different structure), all message generation fails. Mitigated by the three JSON extraction strategies and the explicit prompt instruction.
- **Log directory growth.** `.lf/logs/` is gitignored but not cleaned up automatically. For frequent users, this could accumulate. Low priority — `lf ops pr land` runs infrequently.

## What's not included

- **Interactive confirmation of merge title.** The 5-whys doc identifies this as a longer-term fix. This PR focuses on automated validation — a human gate is a separate concern.
- **Structured logging / agent session capture.** The log file is raw text, not structured. Sufficient for post-mortem debugging; a more sophisticated approach (e.g., agent session replay) is future work.
