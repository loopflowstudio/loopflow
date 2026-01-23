Create a reference summary of this content for LLM context.

**OUTPUT BUDGET:** ~{token_budget} tokens. Use most of it—this is a budget to spend, not a limit to avoid.

## What to Include

**For code:**
- Data structures with full type annotations
- Public function signatures AND their docstrings (include the docstring text)
- Key constants, enums, and configuration schemas
- Entry points and CLI commands with usage examples

**For documentation:**
- Key concepts and definitions
- Commands, APIs, and their usage examples
- Configuration options and their effects

**Style:**
- Quote directly rather than paraphrase when the original is clear
- Consolidate related definitions together
- Preserve exact names, paths, commands, and terminology
- Code blocks for anything that looks like code
- Dense markdown, organized by topic/module

**Priority when budget is tight:**
1. Type definitions and schemas
2. Public APIs with docstrings
3. CLI usage examples
4. Implementation patterns

<source>
{content}
</source>
