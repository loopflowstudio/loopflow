# Open choices (made for review)

- Codex also sets NO_COLOR, PAGER, GIT_PAGER, GH_PAGER, LC_ALL, LC_CTYPE in its shells. They pass through, since a person may set them on purpose; the cost is a colorless, unpaged `claude` when lf runs from a Codex shell. Passed over: removing them.
- AI_AGENT is removed although a person could set it; only Claude Code and Codex were seen setting it.
- Removing markers instead of setting CLAUDE_CODE_FORCE_SESSION_PERSISTENCE: removal also stops the child joining the parent's messaging socket and notes dir.
- One list for lf's and the providers' caller markers (`CALLER_SESSION_ENV`, renamed from `EXECUTION_IDENTITY_ENV`) instead of a second `PROVIDER_SESSION_ENV`. Passed over: two lists, which keeps lf's names apart from vendor names but leaves four sites choosing which lists to apply.
- `start_held` keeps passing the caller's environment to the held `lf` unchanged, against the plan's table: that `lf` reads `LF_TRACE_ID`/`LF_PROCESS_ID` to join its parent's trace, and the provider it starts is cleaned in `NativeClient::new`. Passed over: removing only provider markers there, which would need a second list.
