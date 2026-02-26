# Open questions

- `claude` auth flow is implemented with `claude login` + `BROWSER=echo`/`CLAUDE_BROWSER=echo`. This was not validated against a live Claude CLI session in this branch.
- GitHub auth flow assumes `GH_BROWSER=echo gh auth login --web ...` emits a browser URL early enough for `/v0/auth/github` to return immediately.
- Claude disconnect currently removes files in `~/.claude/` whose names look auth-related (`auth`, `token`, `credential`, `oauth`, `session`) and leaves other files intact; if the CLI stores auth under different names, status/disconnect rules may need adjustment.
