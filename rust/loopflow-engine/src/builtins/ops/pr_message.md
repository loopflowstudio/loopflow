Generate a PR title and body for the changes on this branch.

Review the diff against the PR base and summarize what changed and why.

Do not ask questions. If anything is unclear, make the best assumption and proceed.

## Output format

Return a structured response with:
- **title**: lowercase, with optional area prefix (e.g. `llm_http: add structured output`)
- **body**: markdown with headers, code blocks for commands, and bullet lists

## Title style

Titles are lowercase and concise. Use an area prefix when changes are focused on a specific module or feature area. The area can be new or existing.

Examples:
- `llm_http: add structured output for pr messages`
- `pr workflow: add -a flag to commit and push`
- `fix worktree cleanup on branch delete`

## Body style

Use markdown headers to organize the body. Open with a "Usage" or "Try it" section showing commands in code blocks. Then explain what changed.

Structure:
1. **Usage section** (header + code block) - how to try it, run it, or see it in action
2. **Summary** - one paragraph explaining what this PR does and why
3. **Changes** (optional) - bullet list of notable changes if helpful

Keep it medium length. Stay high-level; don't enumerate every file.

Example body:
```
## Usage

\`\`\`bash
lf ops pr create -a
lf ops pr update -a
\`\`\`

PR create and update now generate title/body via Claude API instead of reading from .lf/COMMIT. The -a flag adds, commits, and pushes before creating/updating.

## Changes

- New llm_http module for structured LLM responses
- Builtin prompts stored in package, separate from user .lf/ tasks
```
