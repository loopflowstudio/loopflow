# Questions

- Assumption: the current kickoff target is `wave/agent-embedding/02-terminal-embedding.md`, because `01-attention-queue-completion` is already implemented on this branch and terminal embedding is the next wave item in sequence. I left `scratch/agent-embedding-portfolio-view.md` untouched.
- Assumption: terminal-session completion is reported back to `lfd` by an `attach`-time shell wrapper that posts to `/v0/terminal-sessions/:id/complete` with a short-lived completion token. This diverges from the original Swift-callback wording in the design doc, but keeps `lfd` authoritative without building a separate process-exit callback path first.
