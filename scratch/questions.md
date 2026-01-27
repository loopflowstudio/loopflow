# Compress Step Analysis

## Summary

Analyzed all code touched by this branch for reduction opportunities. The code is already well-factored with no clear opportunities for simplification that wouldn't add complexity.

## Areas Examined

### Budget Value Duplication
Budget values (50000, 30000, 20000) appear in:
- `ContextConfig` defaults (context.py:183-185)
- `BudgetConfig` defaults (config.py:74-76)
- `step.py` fallbacks (lines 393-395, 549-551)

**Verdict**: Keep as-is. The step.py fallbacks handle the `config is None` case explicitly. Attempting to consolidate via dictionary unpacking (`**({...} if config else {})`) made the code harder to read without reducing line count meaningfully.

### _run_interactive_step vs _run_step (flow.py)
Both functions (99-187 and 190-282) share:
- Print step header
- Gather prompt components and trim
- Create StepRun and log start/end
- Build command and execute

**Differences**:
- Interactive: Uses `subprocess.run()`, removes API keys from env, passes prompt as argument
- Auto: Uses collector wrapper with Popen, writes prompt to file, supports `--push` flag

**Verdict**: Keep separate. The execution paths are fundamentally different (direct subprocess vs collector wrapper). Extracting common code would require passing many parameters or creating a config object that wouldn't reduce complexity.

### run() vs inline() (step.py)
Both functions have similar config resolution and execution patterns.

**Verdict**: Keep separate. The functions handle different use cases (named steps with frontmatter vs inline prompts). Merging would require many conditionals that wouldn't improve readability.

### _limit_to_budget vs _limit_content_to_budget (context.py)
- `_limit_to_budget`: Operates on `list[tuple[Path, str]]`
- `_limit_content_to_budget`: Operates on `str`

**Verdict**: Keep separate. Different input types serve different purposes. Merging would require type checking that adds complexity.

## Conclusion

No reduction opportunities found. The branch adds clean, well-separated functionality:
- Token budgeting for context sections
- Interactive step support in flows
- Area-scoped document gathering

The code follows existing patterns and doesn't introduce unnecessary abstractions.
