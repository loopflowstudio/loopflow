Generate a PR title and body for the changes on this branch.

Review the diff against the PR base. Make the purpose, why it matters, meaningful
scope, and evaluation path obvious to a reviewer arriving cold.

When Loopflow supplies Task context, it adds the canonical Task title, Linear
link, Task flow, PR sequence, and merge disposition after generation. Do not
invent or repeat those facts from branch names or commit messages.

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

Use markdown headers to organize the body. Lead with concrete evaluation, then
explain why the change matters and the meaningful implementation scope.

Structure:
1. **Evaluate** — commands or steps plus the observable result that proves the change
2. **Why it matters** — one paragraph connecting the behavior to its user or operational consequence
3. **What changed** — the important scope, decisions, or investment; do not enumerate files
4. **Risks / Not included** (optional) — boundaries a reviewer should understand

Keep it medium length. Prefer evidence over claims. Do not manufacture a demo
when the diff only supports a test or inspection path.

Example body:
```
## Evaluate

\`\`\`bash
lf code
lf ship
\`\`\`

Observe that `lf code` stops after implementation and compression, while
`lf ship` owns the gate and PR lifecycle.

## Why it matters

Separating implementation from settlement lets a reviewer inspect a stable
change before any merge operation owns it.

## What changed

- Removed the standalone lint skill
- Moved gate into shipping flows
```
