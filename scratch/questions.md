# Open design assumptions

2026-10-14 — Jack Heart accepted current launch settings on stopped-session resume. Saved identity/account remain attached; historical replay reproduces saved inputs.

- Automatic harness recovery within an existing run keeps its already resolved settings. A new explicit resume resolves current settings. No user request yet calls for hot-reloading a live provider's permissions.

2026-10-14 — Implementation dispositions (reversible choices within Jack Heart's accepted scope):

- Current model settings apply only when their provider matches the saved conversation. Without such a setting, retain the saved model; explicit provider changes cannot reinterpret native history.
- Claude Chrome is explicitly on/off in both transports. [Claude CLI documentation](https://code.claude.com/docs/en/cli-reference) limits turn budgets and native schemas to print mode; terminal launches reject them. Other providers retain Flow-level response validation without claiming native schema enforcement. Other headless providers warn about unsupported turn budgets/native schemas and continue; rejecting old captures would break historical replay.
- Existing account routing keeps isolated native history in its recorded provider home, including a resume requesting `--shared`; fresh launches still honor isolate/shared selection. Account identity remains pinned.
- Ordinary completed disposable Runs are not reopened by headless continuation. Contract fixtures use ongoing Wave conversations to test stopped-session resume without changing Session completion semantics.

2026-10-14 — Entry implementation assumptions within Jack Heart's accepted scope: use `--message TEXT` for a headless next turn on both contextual resume and explicit connect. This removes positional ambiguity with the deleted resume ID. Keep `--try` until the separate caller-mode reduction is decided. No Wave delivery policy changes.

2026-10-14 — Jack Heart's latest direct instruction authorizes queue, then landing with waiting after checks pass; it supersedes the older pursuit seed. See delivery-boundary.md. No delivery approval remains open.
