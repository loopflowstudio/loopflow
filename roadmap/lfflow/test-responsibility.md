# Explicit test responsibility

Steps like `implement`, `gate`, `ship` should explicitly state the responsibility to write tests for new code.

**Problem**: Agents know they *can* write tests but don't always treat it as required. Tests are loaded on demand, not included in context — the responsibility needs to live in the prompt, not rely on context inclusion.

**Solution**: Review code-producing steps and add clear test expectations where missing. "Write tests for what you build" as a standard instruction.

**Effort**: Low (prompt review and edits)
