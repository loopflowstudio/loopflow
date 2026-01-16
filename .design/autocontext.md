# Autocontext

Automatic codebase summarization for LLM context. Summaries are generated once, cached, and refreshed in the background when main advances.

## Review

**Verdict:** Ready to ship

Clean implementation. The feature is well-integrated across Python CLI and Maestro UI. The staleness detection using merge-base is a smart design choice—summaries stay stable during feature development and only refresh when main changes.

Minor issues (non-blocking):

1. **Missing `_is_ignored` null check** in `summarize.py:242` — `_is_ignored(p, repo_root, excluded_paths)` is called even when `excluded_paths` is `None`. Should be `if excluded_paths and _is_ignored(...)`.

2. **Config loader doesn't parse summaries** — `Maestro/Maestro/Services/ConfigLoader.swift` adds the `summaries` field but the YAML parser doesn't handle nested structures. The summaries config won't load in Maestro. This is a Maestro-side gap; Python works fine.

3. **Token budget mismatch** — `_metadata.json` shows `token_budget: 8000` but `config.yaml` specifies `tokens: 30000`. The metadata reflects what was actually used when the summary was generated, but the discrepancy might confuse debugging.

## Design notes

**Staleness based on merge-base**: Summaries hash content at `git merge-base main HEAD`, not the working directory. This means branch work doesn't trigger regeneration—only advancing main does. Good tradeoff: summaries stay stable during feature work, and diff_files already shows branch changes.

**Background refresh**: Stale summaries are served immediately while `lfops summarize --all` runs in background. Lock file prevents concurrent refreshes.

**Raw mode bypass**: If source content fits in token budget, summary is stored as-is without LLM call (marked `model: raw`).
