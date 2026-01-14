# lf cp

Copy file context to clipboard for use with external LLMs.

## Usage

```bash
lf cp                      # copy repo docs + diff
lf cp src tests            # include specific paths
lf cp schema.py            # include a single file
lf cp --no-docs            # exclude .md files
lf cp --no-diff            # exclude git diff
lf cp -e "*.test.ts"       # exclude patterns
```

## Behavior

Gathers prompt components using the same logic as `lf run`, then copies to clipboard. Outputs a token breakdown showing what was copied.

Options:
- Positional args: paths to include as context
- `-e, --exclude`: patterns to exclude
- `-v, --paste`: include clipboard content
- `--no-docs`: exclude repo documentation (.md files)
- `--no-diff`: exclude branch diff

Merges with config.yaml `context` and `exclude` settings.
