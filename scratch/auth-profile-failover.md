# Auth profile follow-up

Land the profile router with two field fixes:

- Claude Code 2.1.210 omits the account email from `claude auth status`.
  Preserve strict mismatch rejection, but use the selected Chrome profile email
  when Claude reports no email. Codex remains strict.
- Adopt an already active Codex ChatGPT OAuth login into an isolated managed
  account without another browser round-trip. Reject API-key-only state and
  install the copied credential atomically with mode `0600`.

Before landing, run the focused auth tests plus Rust format and clippy, and
re-run the managed Codex import against the staging registry.
