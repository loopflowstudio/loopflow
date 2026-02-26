# Open questions

- `CredentialSocketServer` currently implements `POST /auth/{provider}/start` as a best-effort browser open + static verification URI response (GitHub device page, Anthropic/OpenAI key pages). It does **not** yet orchestrate full OAuth/device login completion inside Concerto. If full in-app auth flow is required, we should wire this route to a real provider-specific auth launcher and status tracker.
- `BundledDaemonManager` currently switches to native fallback via a persisted `UserDefaults` flag (`concerto.bundledDaemon.preferNativeMode`) set from Connection Settings. Confirm this persistence behavior is desired versus one-shot fallback.
- Local `xcodebuild test -scheme Concerto` currently fails in this environment with `ConcertoUITests-Runner ... Early unexpected exit` (signal kill before bootstrap) after all unit/integration suites complete. Confirm whether this UITest-runner crash is known local flake or requires a branch fix.
