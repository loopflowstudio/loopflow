# 04: Install Onboarding

`lfd install` guides users through provider auth setup. You don't finish installing without connecting providers.

## What to build

### Interactive auth setup in `lfd install`

After the container image is pulled / daemon is configured, run an interactive auth flow:

```
$ lfd install

Setting up loopflow...

  Pulling container image... done

  Let's connect your accounts.

  Claude - Go to claude.ai/device and enter code: ABCD-1234
  Connected as jack@anthropic.com (Max plan)

  GitHub - Go to github.com/login/device and enter code: 5678-WXYZ
  Connected as @jackdoe

  Codex (optional) - skip? [Y/n]
  Skipped

  Ready. Run `lf up` to start.
```

### Requirements

- At least one agent provider (Claude or Codex) must be connected
- GitHub must be connected (required for PRs, CI, git operations)
- Codex is optional — skip with a keypress
- `--no-interactive` flag skips auth setup (for CI, scripted installs)
- Auth uses the same `ProviderAuthService::start_auth` path as `lfq auth` — tokens go to the DB

### `lfq auth status` improvements

Show richer info now that tokens are in the DB:
- Provider name, login, plan/tier if known, token expiry countdown
- "Refresh scheduled in Xm" when the background task is tracking it

## Constraints

- Don't block install on optional providers. Guide through required ones, offer to skip the rest.
- The onboarding flow is CLI-only (stdin/stdout). No browser auto-open requirement — device flow works in SSH sessions.

## Validation

```bash
lfd install              # interactive, walks through auth
lfd install --no-interactive  # skips auth, exits cleanly
lfq auth status          # shows connected providers with expiry info
```

## Done when

- `lfd install` prompts for Claude + GitHub auth before completing
- Optional providers can be skipped
- `--no-interactive` bypasses auth setup
- `lfq auth status` shows token expiry and refresh schedule
