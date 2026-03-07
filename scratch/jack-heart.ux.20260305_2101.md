# Provider auth redesign

## Problem

Concerto currently treats provider auth as one generic "Connect" action, but the underlying systems are not generic:

- GitHub is mostly device-code / existing `gh auth` state.
- Claude can be existing local Claude Code state, browser auth, API key, or terminal-mediated flows that emit instructions instead of a clean callback.
- Codex can be existing local Codex state, ChatGPT device auth, or API key.
- OpenCode Zen is API-key style.

Today we mix four concerns in one path:

1. Detecting already-authenticated local tools.
2. Launching a new auth flow.
3. Persisting/importing resulting credentials into lfd.
4. Rendering status in the UI while the bundled daemon is still starting.

That makes the UI look broken even when the backend is correct, and it makes provider-specific oddities (Claude pasteback, Codex account settings prerequisites) surface as dead-end generic flows.

## Current architecture pressure points

The code currently bakes the wrong abstraction in at several layers:

- `AuthFlow` only models browser/device-code style fields:
  - `verification_uri`
  - `verification_uri_complete`
  - `user_code`
  - `expires_in`
- `AuthProviderStore.connect()` assumes every provider can:
  1. start one flow
  2. launch a browser
  3. wait for a pending state
- `AuthProviderCard` only renders three states:
  - connected
  - pending browser/device flow
  - disconnected
- the macOS bundled `CredentialSocketServer.startAuth()` currently just opens a fixed URL per provider, which is not a real auth protocol:
  - GitHub → `https://github.com/login/device`
  - Claude → Anthropic API keys page
  - Codex → OpenAI API keys page

That is auth theater, not authentication orchestration.

## What comparable products do

### Conductor — detect local auth, don't own the ceremony

Conductor explicitly tells users to authenticate in the terminal environment first:

- installation docs say Conductor requires `gh auth status`
- if you already use Claude Code, "you're all set"; otherwise run `claude /login`
- the product FAQ says it uses Claude Code however you're already logged in

This is the safest shape for providers whose auth is already owned by a CLI.

### OpenCode — one command surface, provider-specific flows

OpenCode keeps the UX inside one command surface (`/connect` or `opencode auth login`), but it does not force one auth protocol:

- credentials are stored in one local auth file
- provider login methods differ by provider
- Anthropic offers:
  - Claude Pro/Max browser auth
  - create API key
  - manually enter API key
- OpenAI is API-key entry
- GitLab offers OAuth or personal access token
- OpenCode also exposes `opencode auth list`

The useful pattern is not "one universal Connect button". It is "one place to start auth, but a typed method per provider".

### Sculptor — explicit provider account management

Sculptor's settings are explicit about credential family:

- onboarding says you need Anthropic or OpenAI credentials
- Claude Max/Pro uses buttons that auth via the Claude website
- account settings split Claude access from OpenAI access
- Codex is API-key only in docs

The useful pattern is honesty: different providers support different methods, and the settings UI says so directly.

## Provider constraints from primary docs

These matter because our UI should mirror the actual provider contracts, not our wishful abstraction.

### Claude Code

Anthropic documents multiple auth types:

- Claude.ai account login
- Teams / Enterprise login
- Claude Console credentials
- cloud-provider auth through env vars

And the first-run flow is terminal-owned: run `claude`, it opens a browser, and if the browser does not open you copy the login URL from the terminal. Credentials are managed by Claude Code itself and stored in the macOS Keychain on macOS.

Implication: Concerto should usually detect Claude Code auth or launch a terminal-assisted Claude Code login, not pretend we can complete every Claude auth variant inside our own sheet.

### Codex

OpenAI documents two Codex auth methods:

- Sign in with ChatGPT
- Sign in with an API key

For the CLI, ChatGPT login is the default. The browser returns an access token to the CLI, login state is cached locally, and on headless / callback-blocked setups OpenAI explicitly recommends device-code auth (`codex login --device-auth`). It also documents the local auth cache at `~/.codex/auth.json`.

Implication: Concerto should treat Codex as a real multi-method provider:

- detect local cached login
- support browser callback if we can safely own it
- support device code as a first-class path
- support API key entry

### GitHub

Conductor's setup docs lean on existing `gh auth` state rather than reinventing GitHub auth in-app. That aligns with the practical reality that many local developer tools already depend on `gh`.

Implication: detection of `gh auth status` should be the default path, with an optional device/browser flow only if we can complete it robustly.

## What we should change

### 1. Split detection from login

Provider cards should always start from a read-only detection pass:

- `detected_local_auth`
- `imported_into_lfd`
- `connected_for_agent_runs`
- `last_verified_at`

The primary user question is not "did Concerto complete a flow". It is:

- can this provider run right now?
- if yes, where is that auth coming from?
- if not, what exact step is missing?

### 2. Replace generic `Connect` with provider-specific actions

Each provider should declare supported acquisition methods, for example:

- GitHub
  - Detect existing `gh auth`
  - Device code in app
  - Open terminal login
- Claude
  - Detect existing Claude Code login
  - Import API key
  - Open terminal login
  - Browser login only if we have a structured callback we fully control
- Codex
  - Detect existing Codex login
  - Device code in app
  - Import API key
  - Open terminal login
- OpenCode Zen
  - API key only

The UI should say the actual method, not a fake universal "Connect".

### 3. Introduce an auth capability model in lfd

Instead of treating every broker as "start auth, then poll status", define broker capabilities:

- `detect_only`
- `device_code`
- `browser_oauth_callback`
- `terminal_login_monitor`
- `api_key_entry`
- `external_settings_prerequisite`

And return a typed start response:

- `DeviceCodeStep { url, code, expires_at, prerequisite_message? }`
- `BrowserStep { url, callback_kind, local_callback_port? }`
- `TerminalStep { command, human_instructions, detection_targets }`
- `ApiKeyStep { placeholder, help_url, validate_on_submit }`
- `PrerequisiteStep { title, body, action_url?, continue_action }`

This keeps UI logic declarative and eliminates provider-specific stdout scraping in the view layer.

### 4. Stop using unstructured CLI stdout as the primary protocol

Current Claude behavior proves this: CLI output can ask the user to paste a token back into the terminal, which our settings sheet cannot satisfy.

Safer rule:

- If a provider flow is not machine-verifiable and machine-completable, do not present it as an in-app connect ceremony.
- Offer `Open in Terminal` and monitor the known auth files / status commands for completion.

This still gives users a one-click path, but the interaction happens in the environment the provider actually supports.

### 5. Add an explicit terminal-assisted flow

For Claude especially, the right UX is probably:

- user clicks `Sign in with Claude Code`
- Concerto opens a terminal window running `claude auth login`
- card switches to `Waiting for Claude Code login…`
- daemon watches `claude auth status` or `~/.claude/.credentials.json`
- card flips to Connected automatically when detected

No paste box in Concerto. No guessing.

### 6. Make device-code flows first-class in the app

For GitHub and Codex, when the provider truly supports device code, the card should become a dedicated stepper:

- open browser button
- large copyable code
- progress / timeout
- explicit prerequisite message if detected from stderr or provider response
- fallback `Continue in Terminal`

### 7. Unify status reads around one source of truth

The settings screen, repo window, and background refresh must all read provider auth from the same daemon endpoint (`/v0/auth`) using the same bundled daemon connection.

No separate cached provider state in the UI without explicit timestamps.

### 8. Surface provenance and risk

A connected provider should show where auth came from:

- `GitHub CLI`
- `Claude Code login`
- `ChatGPT device auth`
- `API key`

And where relevant, a subtle warning when a flow is unofficial / fragile.

## Proposed UI

Each provider card becomes:

- status badge
- credential provenance
- primary CTA based on provider capabilities
- secondary actions: Refresh, Disconnect, Open terminal, Use API key
- expandable troubleshooting details

Examples:

- `Claude — Connected via Claude Code login`
  - Primary: Refresh
  - Secondary: Reconnect in Terminal, Use API Key, Disconnect

- `Codex — Not connected`
  - Primary: Sign in with ChatGPT
  - Secondary: Open in Terminal, Use API Key

- `GitHub — Connected via gh auth`
  - Primary: Refresh
  - Secondary: Reconnect, Disconnect

## Proposed API shape

### Status

Replace the current status payload with something closer to:

```json
{
  "provider": "claude",
  "status": "active",
  "credential_type": "oauth",
  "provenance": "claude_code_login",
  "login": "jack@example.com",
  "last_verified_at": "2026-03-06T22:10:00Z",
  "available_methods": [
    "detect_local",
    "terminal_login_monitor",
    "api_key_entry"
  ]
}
```

### Start flow

Replace `AuthFlow` with a tagged enum / discriminated union:

```json
{
  "provider": "codex",
  "step": {
    "kind": "device_code",
    "url": "https://...",
    "code": "ABCD-EFGH",
    "expires_at": "2026-03-06T22:20:00Z",
    "prerequisite_message": "Enable device code login in ChatGPT security settings first."
  }
}
```

Other variants:

- `browser_callback`
- `terminal_login`
- `api_key_entry`
- `prerequisite`

## Concrete implementation plan

### Phase 1 — make the current system honest

- fix all auth status reads to come from the live bundled daemon
- add `provenance` to `/v0/auth`
- replace generic `Connect` copy with provider-specific actions
- add `Open in Terminal` for Claude and Codex
- stop using `CredentialSocketServer.startAuth()` as a fake provider URL opener

### Phase 2 — add typed auth methods

- replace `AuthFlow` in Swift and Rust with a tagged step model
- update `AuthProviderStore` to manage `PendingAuthStep` instead of browser-only pending flows
- build reusable UI components for:
  - device code
  - browser callback
  - terminal-assisted login
  - API key entry
  - prerequisite gates

### Phase 3 — provider-specific brokers

- GitHub broker
  - detect `gh auth status`
  - optional terminal/device-code login
- Claude broker
  - detect `claude auth status` / keychain / known local state
  - terminal-assisted `claude` / `/login`
  - API key entry / helper support
- Codex broker
  - detect `~/.codex/auth.json` / keyring
  - browser callback where possible
  - device-code step
  - API key entry

### Phase 4 — robustness

- timestamped verification
- polling cancellation and timeout UX
- explicit stale-state recovery when bundled daemon port changes
- event stream for provider auth changes instead of relying only on ad hoc refreshes

## Proposed implementation phases

### Phase 1: reliability

- Fix settings to always read live `/v0/auth` from bundled daemon.
- Add provenance to auth status DTOs.
- Add terminal-assisted auth method support.
- Change broken generic Connect buttons to provider-specific actions.

### Phase 2: structured auth

- Replace ad hoc broker start responses with typed capability-driven steps.
- Build a reusable device-code UI component.
- Add explicit timeout / retry / fallback handling.

### Phase 3: polish

- Add last-verified timestamps.
- Add richer troubleshooting copy per provider.
- Add analytics on where users get stuck.

## Decision

We should not keep trying to make every provider fit one in-app OAuth button.

The robust model is:

- detect local auth reliably
- use structured in-app device/browser flows only where the provider truly supports them
- use terminal-assisted flows everywhere else
- expose provider-specific truth instead of generic auth theater

## Sources

- Conductor install docs: https://docs.conductor.build/installation
- Conductor providers guide: https://docs.conductor.build/guides/providers
- Conductor site FAQ: https://www.conductor.build/
- OpenCode providers docs: https://opencode.ai/docs/providers/
- OpenCode CLI docs: https://opencode.ai/docs/cli/
- OpenCode troubleshooting docs: https://opencode.ai/docs/troubleshooting/
- Sculptor getting started: https://docs.imbue.com/getting-started
- Sculptor models docs: https://docs.imbue.com/core-concepts/core-concepts/models
- Sculptor changelog: https://docs.imbue.com/changelog
- Claude Code authentication docs: https://code.claude.com/docs/en/authentication
- Claude Code getting started: https://code.claude.com/docs/en/getting-started
- Codex auth docs: https://developers.openai.com/codex/auth
- Codex CLI docs: https://developers.openai.com/codex/cli
