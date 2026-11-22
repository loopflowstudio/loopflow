Read when the change touches how Codex launches or what settings it gets.

# Codex launch settings

Also isolate `CODEX_HOME`; set `cli_auth_credentials_store = "file"` in its
disposable `config.toml` to keep probes away from the account's keychain.
Start the branch's `lf -i -a codex : "Reply only with QA_OK"` in a PTY. A PTY
driver must answer Codex's cursor-position query (`ESC[6n`, reply `ESC[1;1R`).
Save the terminal transcript, including sign-in or startup failures.

The private store's `agent_sessions.provider_thread` and joined
`processes.endpoint` identify the launched provider. Read them without changing
rows. Connect a WebSocket over that Unix socket with URI `ws://localhost`, send
`initialize` with `clientInfo` and `capabilities.experimentalApi: true`, then
`initialized`. Real Codex supports `thread/resume` with only `threadId` to read
the tier, and `thread/settings/update` to change `serviceTier`. Reattach through
`lf --setting codex.service_tier=default resume SESSION`; compare the thread,
endpoint, provider PID and tier before and after. Stop and reap private children.

Unauthenticated Codex can create, fork and configure threads through this API,
but its terminal stops at sign-in. Label these provider API checks; they do not
establish authenticated replies or typed `/new`, `/fork` and `/fast` behavior.
Use different names for a headless Graph and its skill so Graph resolution does
not treat the step as a recursive Graph call.
