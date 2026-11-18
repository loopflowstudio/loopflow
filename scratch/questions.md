# Open choices (made for review)

- Codex also sets NO_COLOR, PAGER, GIT_PAGER, GH_PAGER, LC_ALL, LC_CTYPE in its shells. They pass through, since a person may set them on purpose; the cost is a colorless, unpaged `claude` when lf runs from a Codex shell. Passed over: removing them.
- AI_AGENT is removed although a person could set it; only Claude Code and Codex were seen setting it.
- Removing markers instead of setting CLAUDE_CODE_FORCE_SESSION_PERSISTENCE: removal also stops the child joining the parent's messaging socket and notes dir.
