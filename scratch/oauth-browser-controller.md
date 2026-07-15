# OAuth browser controller

> "This project was supposed to set up a chrome control agent so that I didn't
> have to do it manually."

## What to build

Let managed Claude account login use a narrow Claude-in-Chrome controller to
authorize the provider page and return its one-time handoff directly to the
waiting Claude CLI. Preserve a hidden terminal prompt only when no connected
browser controller exists.

On macOS, capture the credential blob that Claude writes to its single global
Keychain entry into the target mode-0700 profile immediately after login.
`CLAUDE_CONFIG_DIR` does not isolate that Keychain entry by itself.

## The demo

```sh
lf auth connect claude --account reserve
lf auth accounts claude
```

Chrome opens the authorization page, the controller clicks **Authorize**, and
`reserve` appears without a code reaching the terminal or chat. Running
`CLAUDE_CONFIG_DIR=~/.lf/accounts/claude/reserve claude auth status` remains
active after the global Keychain credential changes again.

## Data structures

```rust
enum AuthorizationCodeSource {
    Chrome,
    Terminal,
}

struct BrowserAuthorization {
    code: SecretString,
}
```

The controller result is process-local. It is never added to a DTO, database,
argv, trace, or log.

## Key functions

```rust
async fn drive_claude_authorization(
    verification_url: &str,
    controller_profile: Option<&Path>,
) -> Result<Option<SecretString>, AuthError>;

fn capture_claude_keychain_credentials(profile: &Path) -> Result<(), AuthError>;
```

`drive_claude_authorization` runs `claude --chrome --print` with only the
built-in skill loader and exact Chrome tab/navigation/read/click tools
preapproved. Its structured output must contain one value matching Claude's
`code#state` handoff shape. Missing extension connectivity returns `None` so the
CLI can fall back to its no-echo terminal prompt; malformed or verbose output
is rejected.

`capture_claude_keychain_credentials` reads service
`Claude Code-credentials` directly into target `.credentials.json` with mode
0600 and validates the JSON shape without printing it. Non-macOS homes already
receive the provider-native file and need no copy.

## Constraints

- The official Claude extension must be installed, signed in, granted browser
  permissions, and connected once. Loopflow cannot bypass that bootstrap.
- The controller may authorize an already signed-in account. Account login,
  MFA, CAPTCHA, account creation, purchases, and recovery remain human actions.
- Chrome control is restricted to Anthropic authorization/callback origins.
- The target authorization code and Keychain blob never reach stdout, stderr,
  logs, SQLite, traces, shell argv, or LLM prose.
- The manual input pipe remains as a recovery path.
- Gstack is not a dependency; Loopflow skill compilation must not imply that
  its removed runtime is installed.

## Done when

1. A fake controller test proves a valid structured handoff reaches the waiting
   auth child and malformed output cannot.
2. A macOS credential-capture test proves a target profile receives mode-0600
   credentials without logging content.
3. Existing cancellation and manual-code tests remain green.
4. A live connected Chrome run completes one second account without a pasted
   code.
5. `cargo fmt --all -- --check`, provider-auth tests, and
   `cargo clippy -p loopflow --all-targets -- -D warnings` pass.
