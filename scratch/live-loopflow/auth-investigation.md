# Live Flow Session auth investigation

Main's subsequent live probe identified the cause after this read-only audit:
the stored OAuth injection makes native `codex login status` fail with
`agent identity JWT payload is not valid JSON`. Removing only
`CODEX_ACCESS_TOKEN` at the actual provider boundary allowed native publication
and a ready review. The ordinary stored-token mapping is now being repaired;
the live probe and final verification are recorded in contract.md. The audit
below preserves what was known before that experiment.

2026-09-25. Bounded read-only investigation for LOO-295. Only this note was
written. Main owns failed-boundary recovery. No provider, Session, auth command,
demo, installation, Git/PR/PM mutation, credential-file read, Keychain lookup,
or credential copy was performed. Source was changing concurrently; symbols
below identify the inspected paths more reliably than line numbers.

## Finding

The exact cause of Codex's sign-in screen is **not established** by the retained
evidence. Shell login status and `lf auth status codex` do not prove the auth
context of the failed Ghostty child. Source establishes how those contexts can
differ, but it does not establish which difference occurred on that launch.
There is no evidence here supporting a blanket file-store override, another
login, or an account import as the fix.

The smallest supported retry is the same saved Session, after main's unpublished
Run recovery repair, with an explicit development executable/Home and the known
ambient Codex Home supplied **inside the terminal's executed command**. Clearing
variables only in the process requesting a GUI window does not establish the
environment of the command that window executes. First compare a credential-free
environment probe through that exact terminal handoff; do not consume another
provider launch merely to inspect environment.

## Observations

- `contract.md` reports shell login success both with the ambient `CODEX_HOME`
  and with it unset, followed by a sign-in screen and the 30-second Session
  publication timeout in Ghostty. Its second attempt stopped at missing native
  history before another provider launch. That second attempt therefore does
  **not** test whether its environment cleanup repaired authentication.
- The failed manifest remains at
  `/Users/jack/.lf-dev/worktrees/loopflow-restore-task-continuation-with-a-96ef79ac0e17/runs/66/run_6672e27de94e493799dd69efc516fa9e/manifest.json`.
  Selected-field inspection confirms `harness: codex`, `surface: tui`, no explicit
  model, this checkout as cwd, and `launch: null`. Its directory contains the
  manifest, context, events and provider-client directory, with no native
  provider-session receipt. No prompt/event body or credential value was read.
- **`launch: null` is normal for this TUI path.**
  `lf/commands/run.rs::begin_run_capture` calls
  `CaptureHandle::begin_with_context` for TUI and only constructs a launch request
  for headless execution. It is not evidence that a provider never spawned.
  The timeout and missing native receipt establish failed publication; neither
  alone identifies its auth cause or grants process teardown authority.
- This investigation's shell has
  `CODEX_HOME=/Users/jack/.lf/accounts/codex/engineering`, no `LF_HOME`, no
  `LF_BIN`, and no `LF_ACCOUNT_LEASE`. `CODEX_ACCESS_TOKEN` and `OPENAI_API_KEY`
  are absent; `LF_PROVIDER_ACCOUNT_ID` is present. These are present-tense
  observations, not a reconstruction of Ghostty's former child.
- Shell resolution finds `/Users/jack/.local/bin/codex`, a symlink to the
  standalone Mach-O binary under `~/.codex/packages/standalone/current/bin`.
  There is no shell wrapper at that resolved path. Ghostty's failed PATH was
  not captured, so identical executable resolution is unproven.
- `auth.json` exists under both `~/.codex` and the engineering Codex Home.
  Only existence was checked. Engineering's `config.toml` resolves to
  `~/.codex/config.toml`. Parsed inspection found no top-level
  `cli_auth_credentials_store`, `forced_login_method`,
  `forced_chatgpt_workspace_id`, selected profile or model provider; no profile
  declares an auth-store mode. No ancestor/checkout `.codex/config.toml` was
  found by that check. System/MDM requirements were not inspected. File existence
  does not prove usable credentials, and an absent setting does not prove the
  effective mode of the failed process.

## Source trace

1. `ops/flow_session.rs::open` selects the current Home executable and starts
   `lf --tui ... skill <captured-skill> <review-message>` in the saved cwd. It
   sets exact human/Flow tokens, but does not pin `CODEX_HOME` or clear inherited
   provider routing variables. The saved Flow captures model and Work selectors,
   not an ambient credential Home.
2. `ops/human_session.rs::spawn_session_run` supplies a temporary Run-binding
   path and starts that child with kill-on-drop. It waits for both a native
   provider-session receipt and an owned active client, checking child exit and
   the 30-second deadline. It does not select credentials or open Ghostty.
3. `lf/commands/run.rs::launch_prompt` forces the TUI path, begins a Run, then
   calls `util::launch_session_with_env` with capture identity. Capture environment
   contains Run/Run-directory and optional parent identity, not provider settings.
   `begin_run_capture` publishes the Run binding and calls `flow_run::bind_run`
   **before** native SessionStart publication. This explains how a failed startup
   can leave a bound Run; main's recovery repair addresses that separately.
4. `lf/commands/util.rs::session_command_status_with_env` resolves an account,
   builds `codex`, adds the SessionStart hook, applies stored provider environment,
   applies a selected account route if any, then spawns the provider in place.
   The hook invokes this executable's `__provider-session` command. There is no
   Ghostty-specific auth conversion in this path.
5. `provider_account.rs::resolve_provider_account_exact` consults an inherited
   account lease first, then the selected registry. Without a lease, exact account
   demand or configured accounts, it returns `None`. In that case the TUI does
   not override `CODEX_HOME` and does not append the file-store setting. The fact
   that the branch Home has no managed accounts rules out managed routing only
   when that is the registry actually selected and no lease changes the route.
6. For a Local/Direct route, `ProviderAccountRoute::apply` clears provider credential
   variables and sets its `CODEX_HOME`; TUI adds
   `-c cli_auth_credentials_store="file"`. A Lease route instead supplies a token
   and clears the inherited Codex Home. The same native-home/file-store condition
   exists in `engine/agent.rs` and `harness/codex.rs`; it is not unique to TUI.
7. `provider_auth::apply_provider_env_to_command` removes ambient API keys for
   Codex and may inject a stored Codex OAuth token as `CODEX_ACCESS_TOKEN`, or a
   stored API key as `OPENAI_API_KEY`. It does not remove ambient `CODEX_HOME`.
   **No managed accounts does not mean no stored provider token.** Those are
   separate tables/surfaces. Stored token contents were not inspected here.
8. `lf auth status codex` uses `local_auth_service` and
   `ProviderAuthService::resolve_snapshot`. A stored token's metadata can satisfy
   status before any native check. Otherwise the default `CodexAuthBroker` reads
   `~/.codex/auth.json`, ignoring ambient `CODEX_HOME` and native Keychain mode;
   a credential socket can supply a different broker. Status can refresh and
   auto-persist a token. It is neither a pure read nor an exact TUI-auth preflight,
   which is why it was not rerun for this investigation.

Development `database_path_from_env` ignores `LF_CONTROL_*` for registry
selection, while respecting ordinary overrides; observability does honor control
Home. Thus stale `LF_CONTROL_HOME` alone is not proof of wrong provider-account
routing. Stale ordinary `LF_HOME`/`LF_DB_PATH`, a forwarded lease, provider Home,
stored-token injection, or a different executable are distinguishable possibilities.

Official Codex documentation confirms that `file` uses `CODEX_HOME/auth.json`,
`keyring` uses OS storage, and `auto` prefers OS storage with file fallback.
It also documents login restrictions and administrator-enforced auth settings.
This establishes why the effective mode matters, not which mode this failed
binary used. [OpenAI authentication documentation](https://learn.chatgpt.com/docs/auth#credential-storage).

## Hypotheses and discriminating evidence

| Hypothesis | Evidence and limit | Smallest next discriminator |
| --- | --- | --- |
| Terminal child inherited a different Home, route or executable | Contract observed stale Ghostty LF variables; Flow/TUI launches inherit ambient context. Exact failed child context is absent. | Through the same terminal command mechanism, report only resolved executable path, cwd, Home paths and presence/absence of routing/token variables. Compare with the known shell. |
| File versus native credential-store mismatch | Mechanism exists for managed native-home routes. Clean no-account path does not add the override; inspected configs give no positive evidence of a mismatch. | Establish route kind and effective auth-store setting first. Do not force file mode globally. |
| Stored token injection differs from bare CLI | Source proves this independent path, and prior lf status can populate it. No token validity/type comparison was performed. | In an isolated regression, use a fake Codex executable to observe variable names/presence and route behavior; never print token values. A real validity check requires separate authority. |
| Expired/rejected credentials or native configuration | Sign-in is compatible with this; successful shell status and shared config weaken a simple universal auth-failure explanation. | After matching launch context, test native login status only if authorized to invoke credential access; this investigation prohibits it. |
| SessionStart capture failure | Explains missing native receipt and timeout, but does not explain the reported sign-in screen by itself. | On the eventual authorized retry, distinguish authenticated TUI startup from hook publication; inspect native receipt and owned client separately. |

## Supported recovery and fixes

For main's later authorized retry, keep invocation
`41729e12-84b5-4ee8-9ca9-4ae836a20b76` and boundary
`bd6c5338-d629-4908-8078-b92b1119ef7b`. Use the absolute branch `target/debug/lf`,
explicit branch Home/database, and explicit engineering `CODEX_HOME` in the
command Ghostty executes. Remove stale execution/control/lease context there,
preserving the selected provider Home. Do not import accounts, copy credentials,
install the branch or manually edit the saved Flow record. The cleaned second
attempt must reach provider startup before it can count as an auth recovery test.
None of these steps was executed here.

Two code conclusions are supportable:

- **Required continuation repair:** retry a proven-unpublished failed human Run
  under the existing launch lock and ownership rules; preserve an actually
  published native identity and never turn retry into approval. Main is already
  implementing this; an added `recover_unpublished_run` call appeared during the
  final source read. This note neither edits nor validates it.
- **Concrete auth-status defect:** the ambient Codex broker hardcodes `~/.codex`
  while the no-route TUI honors `CODEX_HOME`. Make native ambient status use the
  same Home and configured native store as ambient launch, keeping managed
  profiles' explicit file-store policy. Merely changing the path while continuing
  to read `auth.json` directly leaves Keychain status inconsistent. Keep generic
  stored-token status distinct from a launch-auth check; do not describe one as
  proving the other. This fixes misleading diagnosis but is not yet proven to
  fix the reported sign-in.

If the terminal environment probe shows drift, fix the handoff's explicit
environment construction at its owner; do not add a global auth fallback or
propagate every credential variable. The inspected ordinary TUI path does not
itself create the external Ghostty window, and this investigation did not recover
the original window-creation command. Choosing that fix before observing the
handoff would be guesswork.

Proposed regression: a fake Codex executable in an isolated Home records only
the received Home, whether a file-store argument is present, and token-variable
presence. Cover ambient custom Home with no accounts, a managed local route,
and matching status/launch configuration. Exercise the actual CLI launch adapter
rather than asserting mock calls. No tests were run here; live authenticated
startup, native publication, human approval and the remaining demo are still owed.
