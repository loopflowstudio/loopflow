# Profile-first provider auth

`lf auth connect <provider> --profile <email>` is the normal managed OAuth
flow. The profile selects the browser identity and Loopflow derives, reuses,
and binds the provider account.

Remove `--account` and direct Chrome selection from managed connection. Bare
`lf auth connect <provider>` retains the ambient single-account flow.

## Contract

- A mapped profile reconnects its mapped account.
- An unmapped profile reuses the unique provider account whose login email
  matches the profile.
- A new login receives a deterministic internal account id and is bound to the
  profile only after OAuth succeeds.
- A shared mapped account authenticates through the Chrome binding matching
  the account's login email, not through a different consuming profile.
- Provider login and chosen Chrome identity must match.

## Done when

`lf auth connect codex --profile operator@example.com` connects and binds an
account, while `auth connect --account` is rejected.
