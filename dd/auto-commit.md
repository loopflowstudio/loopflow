# Auto-commit handling

> "I want to make sure we add a special header like '# loopflow auto-commit' or something that we can easily roll back against"

## What to build

When running `lf claude` in non-interactive (print) mode, loopflow makes the commit itself with a consistent header, using a summary provided by the LLM.

## Data structures

```python
COMMIT_MESSAGE_FILE = ".lf/commit-message.txt"
COMMIT_HEADER = "# loopflow auto-commit"
```

## Flow

1. Task file instructs LLM: "Write your commit summary to `.lf/commit-message.txt`. Don't commit."
2. LLM does work, writes summary to that file
3. `lf claude` (in print mode) finishes, then:
   - Checks if `.lf/commit-message.txt` exists
   - If yes: stages all changes, commits with header + message contents, deletes the message file
   - If no: no commit (task didn't request one)

## API

```python
def auto_commit_if_requested(repo_root: Path) -> bool:
    """Commit with auto-commit header if message file exists. Returns True if committed."""
    msg_file = repo_root / ".lf" / "commit-message.txt"
    if not msg_file.exists():
        return False

    summary = msg_file.read_text().strip()
    full_message = f"{COMMIT_HEADER}\n\n{summary}"

    # git add -A && git commit -m "..."
    ...

    msg_file.unlink()
    return True
```

In `cli.py`, after `launch_claude()` returns in print mode:

```python
if print_mode:
    exit_code = launch_claude(prompt, print_mode=True, cwd=repo_root)
    if exit_code == 0:
        auto_commit_if_requested(repo_root)
```

## Constraints

- Only in print mode (non-interactive). Interactive sessions manage their own commits.
- Header must be exactly `# loopflow auto-commit` for easy `git log --grep` and rollback scripts.
- Delete the message file after committing so it doesn't persist.
- Add `.lf/commit-message.txt` to `.gitignore`.

## Task file updates

Update `review.lf` and `implement.lf` to:
- Write summary to `.lf/commit-message.txt` instead of committing directly
- Remove "don't commit" language since loopflow handles it

## Done when

```bash
# Run a task in print mode
lf claude review -p

# Check the commit
git log -1 --format="%s%n%b" | head -1
# Should show: # loopflow auto-commit

# Roll back all auto-commits
git log --grep="# loopflow auto-commit" --oneline
```
