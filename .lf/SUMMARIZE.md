Create a reference summary of this content for LLM context.

**OUTPUT BUDGET:**
- ~{token_budget} tokens (~{char_target} characters, ~{word_target} words, ~{line_target} lines)

This is a budget to USE. Include all useful content—if the source material is rich, use most of the budget. If the content summarizes cleanly in fewer tokens, that's fine too.

**What to include**: Full dataclass/model definitions with type hints. Function signatures with docstrings. Key configuration schemas. Important documentation passages quoted directly rather than paraphrased.

## What to Include

**For code:**
- Data structures with full type annotations (quote the dataclass/model definitions)
- Public function signatures with docstrings
- Module organization and file purposes
- Key constants, enums, and configuration schemas

**For documentation:**
- Key concepts and definitions (quote important passages)
- Commands, APIs, and their usage examples
- Configuration options and their effects
- Architecture decisions and rationale

**For any content:**
- Preserve exact names, paths, commands, and terminology
- Quote directly rather than paraphrase when the original is clear
- Include code blocks for anything that looks like code

## Format

Dense markdown. Organize by topic/module. Use code blocks liberally.

**Priority order when budget is limited:**
1. Type definitions and schemas (direct quotes)
2. Public APIs and commands
3. Key documentation passages
4. Implementation patterns

<source>
{content}
</source>
