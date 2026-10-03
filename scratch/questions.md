# Remaining design uncertainties

## For Jack

- Command name: built as `lf account use <provider> <account>`, since accounts live under `lf account` and there is no `lf auth`. A planning choice, not his.
- Isolation default: `isolate: true` works in the repository and Home `config.yaml`; is a Wave-level default also wanted?
- A native login Loopflow has never seen is stored as an explicit-only account, so automatic routing does not start spending it. Confirm.
- Launch-driven switches (the active account is strained, or a launch names another account) are “from now on” until slice 3 builds “everyone now”.
- “Pinned binary”: only the account's own home, or also a pinned provider or `lf` executable.
- Automatic switch on exhaustion uses “everyone now”; confirm.
- Placement remains unresolved; no Wave supplied.
- Plain `codex resume` is refused while Loopflow's retained engine for that conversation is alive (“already has an active writer”), including after a finished headless run. Should Loopflow stop a finished conversation's engine, so the demo's second line works straight away?
- Only `lf session connect` brings in a conversation plain Codex started; `history`, `rename`, `bind` and `complete` report it as not found until then. Confirm.
- A shared conversation now resumes under the active account rather than switching the home back to the account it began under. Confirm.

## To research

- What most affects how seamless an interrupted session feels (Jack's question): terminal versus headless, unsent input and scrollback, in-flight tool calls running twice, gap length, whether the agent notices, and what history shows. Gates “everyone now” for Codex.

## To prove while building

- Codex: whether refresh tokens rotate such that the active account's stale stored profile and the native home invalidate each other when both refresh. Probes and readiness now read the native home; an `--isolate` launch on the active account still runs on the profile copy.
- Codex terminal `codex resume` against a conversation whose Loopflow engine is alive: refused like `app-server`'s `thread/resume`, or not.

- Claude: a credential written to the Keychain item by `security add-generic-password -U` (macOS) or to `.credentials.json` (elsewhere) is accepted by a subsequently started Claude.
- Claude: a running shared agent adopts the native login, and how soon. Read from the binary only.
- A conversation started under A resumes under B across different workspaces or plans.
- SSH-forwarded Codex: the lease exports `CODEX_ACCESS_TOKEN`, recorded in `provider_auth/mod.rs` as the wrong credential type. Untouched by this plan.

## Assumptions made while building slice 1

- The mode travels in its own `LF_ACCOUNT_ISOLATION` variable and an optional `isolate` field on the Flow invocation, not inside `LF_ACCOUNT_SELECTION`. Changing that variable's encoding broke launches nested under a process started by the installed `lf`, which still exports the old shape.
- An interactive conversation's home is recorded when its provider process exits. If `lf` dies first, reopening follows the launch's mode.
- An unidentifiable native login (an API key, or an unreadable file) is kept as `accounts/codex/native-<digest>.auth.json` rather than refusing the switch.
- Activation serializes on one provider-wide lock and writes profiles by atomic rename; it does not take the per-account managed-login lock.
- A stored profile directory under `<LF_HOME>/accounts` is never treated as the native home, which is how a launch nested under an isolated parent finds the real one.
- Test support now points `CODEX_HOME` at a temporary directory, so a shared launch in a test cannot sign the developer's own Codex home in as a fixture account.
- Switch-log attribution covers rate-limit and credential-health observations, the only per-account usage evidence there is; no per-account usage report was added.
- A provider conversation ID is searched for in provider homes only when it is a UUID.
- An admitted conversation's Session takes the directory its transcript recorded, else the current one, and is titled `<provider> <first 8 of the ID>`.
