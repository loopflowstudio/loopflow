# v0.13.3

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.3 repairs scheduled release settlement after the move to headless publication in v0.13.2. Scheduled releases can now settle using the required public verification without waiting for a retired UI-host receipt, and installer smoke checks use the current CLI discovery command.

## Finish scheduled releases with headless proof

Publisher preparation had stopped requiring the UI host, but scheduled settlement still expected its receipt. This patch brings settlement into agreement with the headless publication path while retaining the required public checks.

- Accept public artifact, website, crate, and installer proof without a UI-host stage.
- Preserve failure handling: removing the obsolete receipt requirement does not remove the required publication evidence.
- Regression coverage verifies published, no-change, and failure scenarios without UI proof.

## Check the installed CLI with a supported command

Public installer smoke checks now run `lf list --json` instead of the removed catalog command. The test fixture also rejects unknown commands, so an obsolete smoke command cannot silently pass again.

## Operational notes

Upgrade the release host to receive the scheduled-settlement repair. The supplied evidence confirms that v0.13.2 was published and installed; it does not establish live scheduled settlement with v0.13.3.

Validation for this patch includes the scheduled-release regression, all 33 publisher tests, formatting, all-target Clippy, and Ruff.