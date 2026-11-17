---
extends: builtin
---
## In this repository

### Driving native providers from a PTY script

A PTY driver must keep reading the master until it reaps the child, including
while it kills one: on macOS a process closing its tty with unread output sits
in exit (`ps` STAT `E`) and `waitpid` never returns. Treat EOF on the master as
"reap now", not as a timeout. Rendered text loses spaces where the TUI moved the
cursor instead: match Claude's trust dialog as `trustthisfolder` (send Down,
then Enter), Codex's prompt as `Ask Codex`, and send Enter only once the typed
line shows. Even `lf -b -a codex` in a new directory makes Codex add a
`[projects."<dir>"] trust_level` stanza to the account's `~/.codex/config.toml`;
remove it afterwards.

`lf -b session resume <id> --message` refuses Runs (`lf -b : …`); resume
Sessions started with `lf -i`. A failed headless resume leaves a capture with no
provider thread, so the next resume of a Pi Session started with `lf -i` opens a
new conversation: check `to_provider_session` in its `transition` row.
