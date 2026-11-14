# Open questions (LOO-902)

- **Real redemption spends a real grant.** The POST to
  `/api/organizations/{org}/reset_rate_limits` has never been sent by lf. Its
  headers, the reply to a repeated `request_id`, and `not_next_grant`
  behaviour can only be confirmed by spending one. On 2026-11-14 the
  `loopflow` login held one usable weekly grant (`full-reset-2611`, ends
  2026-12-10) and one usable Fable grant (`launch-2610-fable-6mo`, ends
  2027-03-09). No unattended step spends either; the first real run is a
  person's choice of login and scope.
- **Early use.** The provider allows a grant to be spent below any limit
  (`use_requires_limit: false`, usable at 18% used). The plan prints the
  percentages and spends when asked. Assumed: no lf-side refusal or
  confirmation prompt.
- **`--credit-id` renamed `--reset-id`**, and the Codex JSON shape for resets
  changes (`reset_credits.availableCount` → `resets.usable`). Assumed fine
  with no compatibility alias.
- **Fable window name on the wire.** `Reset.clears` uses lf's poll names
  (`seven_day_fable`), not Claude's `seven_day_overage_included`. Assumed so
  status lines match the window rows above them.

- **Every Claude usage poll now asks for the reset block**
  (`?cedar_ember=1&skip_spend=1`), including routing's and the at-rejection
  poll, where the first design kept the plain URL. Assumed fine once an
  undecodable block degrades to "not reported" instead of failing the poll.
  What `skip_spend=1` leaves out of the body was not examined; lf decodes only
  `limits` and `cedar_ember`.
- **Scope selection differs by provider.** With no `--scope` or `--reset-id`,
  Claude stops when usable resets span several scopes; Codex lets the provider
  choose. Left as is to keep Codex behaviour.

## Implementation choices (2026-11-14)

- Provider outcomes exit 0 in text and JSON, as the saved docs specify.
  Only an unsent or unconfirmed attempt fails.
- Text uses session / weekly / weekly:fable; JSON retains five_hour /
  seven_day / seven_day_fable. Reset selections and attempt keys print to
  stderr before the POST, keeping JSON stdout parseable.
- `newer_observation_kept` reports when the existing capacity revision
  rejected the after-read. Failed after-reads save no before-state.
- A Claude retry requires both the key and reset ID. It reaches the provider
  even if the grant is now missing or unusable; no automatic replay follows
  a timeout, 429, 5xx or unknown result.
