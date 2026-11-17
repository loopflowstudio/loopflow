Read when the change switches a Session between Claude and Codex.

# Switching a Session's provider

`lf -b : '<prompt>'` records a Run, which `lf resume` refuses. For a real
conversation turn without a PTY, prepare a Session
(`lf -a claude session resume --prepare --json` in the repository) and send
turns with `lf -b [-a <harness>] session resume <id> --message '<text>'`; real
Claude and Codex answer on the account's logins with only `LF_HOME` private.
Each turn appends a capture: read `agent_sessions.provider`/`provider_thread`
and `json_extract(payload,'$.provider')` of `kind='captured'` rows. For the
interactive path, a PTY driver must answer `ESC[6n` and `ESC[5n`; kill the
provider (not its holder) to make the next resume a dead-provider resume. A
plain `lf -i session resume` with the private server stopped fails with
"Loopflow server is not running"; start the branch's `lf server run` first.
A store's provider rows without connected accounts have no
`provider_session_accounts`; the current input's `<key>:manifest.json`
observation (`$.evidence.harness`) is what ran.
