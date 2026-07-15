# Codex native OAuth

`lf auth connect codex --account <account> --profile <profile>` must keep the
provider UI human-owned while making account placement deterministic.

## Contract

- Start Codex's native local-callback OAuth flow in the isolated account home.
- Suppress Codex's default-browser launch and open the captured URL in the
  host-local Chrome directory bound to the Loopflow profile.
- Never print or persist the authorization URL, state, or device code.
- Wait for Codex to receive its callback; do not automate account choice,
  authorization, MFA, CAPTCHA, extension installation, or provider settings.
- Read the authenticated email from the resulting Codex ID token and reject a
  mismatch with the bound profile before registering the account.
- Keep headless authentication out of the normal path. `lf ssh` forwards an
  already-authenticated local account for the process lifetime.

## Done when

- Managed Codex login opens the bound Chrome profile without Claude in Chrome.
- Clicking Authorize completes the CLI flow without copying a device code.
- The account is registered only when its authenticated email matches.
- Missing or expired native accounts leave automatic routing.
