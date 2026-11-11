# Open questions (LOO-844)

- `--history` on `lf session list`: the quote "dont even have have a filter for non-completed.
  thats history suff" may mean the flag leaves the list for `lf history` (LOO-822). Docs keep it.
- Failed headless exits now complete. A retry attaches a new AgentProcess and reopens,
  so a retrying step lists while it runs. Assumed right; revisit if a failed step
  should stay listed for attention.
