Generate a commit message for the staged changes.

Read the diff and write a short message. Don't ask questions; assume and proceed.

## Output format

Return a structured response with:
- **title**: lowercase, with optional area prefix (e.g. `llm_http: add structured output`)
- **body**: brief explanation if needed, otherwise empty string

## Title

Lowercase and short. Add an area prefix when the change sits in one module or feature.

- `llm_http: add structured output for pr messages`
- `pr workflow: add -a flag to commit and push`
- `fix typo in readme`

## Body

Most commits need none. Add one sentence or a few bullets only when the title doesn't explain why.
