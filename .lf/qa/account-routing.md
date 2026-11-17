Read when the change touches provider logins, account capacity or routing.

# Account capacity and routing

A private home gets Claude logins without OAuth or `lf account`: per login, a
`provider_accounts` row (`credential_state='connected'`, `observed_email`,
`observed_subject`, `observed_credential_digest` = sha256 of the exact
`.credentials.json` bytes under `$LF_HOME/accounts/claude/<id>/`) plus
`accounts.claude.<id>: {login_email, routing_state: automatic}` in
`$LF_HOME/config.yaml`. Raw SQL works on a store the installed `lf` created,
so its recorder can write the "before" rows and the branch then upgrades them.
A stand-in `claude` that prints a `rate_limit_event` line between `init` and an
`is_error` `result` records a limit through a real `lf -b -a claude:<model>`.
`--only-account` and `--account` bypass capacity, so use them to record a
limit and plain routing to check it; `lf account --cached` marks the login a
launch switched to `● active`. `-b` runs are not resumable Sessions: launch
`-i` in a PTY (the stand-in prints and exits) and `lf -i session resume`.
The Claude usage endpoint override (`LF_TEST_CLAUDE_USAGE_URL`) exists only
under `cfg(test)`, so a branch binary's poll reaches Anthropic; the poll path
is checked by `cargo test -p loopflow --lib claude_usage_reconciliation_`.
Name the PTY driver anything but `pty.py`: it shadows the stdlib module.
