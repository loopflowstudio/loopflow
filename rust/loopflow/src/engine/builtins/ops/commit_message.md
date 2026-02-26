Generate a commit message for the staged changes.

Review the diff and write a concise commit message.

Do not ask questions. If anything is unclear, make the best assumption and proceed.

## Output format

Return a structured response with:
- **title**: lowercase, with optional area prefix (e.g. `llm_http: add structured output`)
- **body**: brief explanation if needed, otherwise empty string

## Title style

Titles are lowercase and concise. If you are working within a wave (indicated by `<lf:wave>`), always use the wave name as the area prefix. Otherwise, use an area prefix when changes are focused on a specific module or feature area.

Examples:
- `wave-name: add structured output for pr messages`
- `llm_http: add structured output for pr messages`
- `fix typo in readme`

## Body style

Keep it brief—one sentence or a few bullets if the change needs explanation. Empty string is fine for self-explanatory changes.

Most commits don't need a body. Only add one if the "why" isn't obvious from the title.
