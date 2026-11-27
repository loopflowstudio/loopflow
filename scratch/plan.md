# LOO-1118: rentals receive a repository-scoped, expiring GitHub token

## Outcome

A rental never holds the laptop's `gh` token. It holds a GitHub App user
access token (`ghu_…`) that reaches the one repository the rental runs, acts
as the same person (PR author unchanged), and stops working within 8 hours of
issue, sooner when the repository's last rental is given back. The grant that
mints it stays on the laptop, as Linear's does.

A template with no `logins` signs the rental in to GitHub and Linear only.

Rental first contact stops forwarding the SSH agent and X11, Daytona's pinned
gateway key stays pinned, and a route's userinfo no longer appears in errors.

## Demo

```bash
lf set --global forward.github_app <client id>   # once
lf machine login github --rental                 # once per repository: one device code
lf task run LOO-570 --rent code
ssh <rental> 'gh auth token | cut -c1-4'         # ghu_
ssh <rental> 'gh api repos/<owner>/<other-private-repo>'   # 404
ssh <rental> 'cd ~/src/loopflow && git push --dry-run'     # works
```

## Mechanism, and what was passed over

**Chosen (made for review, nobody was here to ask):** a GitHub App with device
flow, one grant per repository kept on the laptop, its access token handed to
that repository's rentals.

Findings (GitHub docs, read 2026-11-26):

- Device flow needs only the app's `client_id`; no client secret, no private
  key. The token exchange takes `repository_id`: "The ID of a single
  repository that the user access token can access." (`generating-a-user-access-token-for-a-github-app`)
- The token "only has permissions that both the user and the app have" and
  "can only access resources that both the user and app can access".
- Access token lasts 28800 s (8 h), refresh token 6 months. Refresh takes
  `client_id`, `grant_type`, `refresh_token`; `client_secret` is "Required
  unless the user access token was generated using the device flow". No
  `repository_id` on refresh.
- "Once you use a refresh token, that refresh token and the old user access
  token will no longer work." So a grant has one live access token.
- Since 2026-10-13 a refresh token is issued only when the device request asks
  `scope=offline_access`.
- `POST /credentials/revoke` takes `ghu_`/`ghr_` unauthenticated but emails the
  owner each time; not used.
- Installation tokens: 1 h, minted with a JWT signed by the app's private key,
  restrictable by `repository_ids` and `permissions`.
- Fine-grained PATs: created in the web UI only, expiry in whole days (1–366).
- No Loopflow GitHub App exists; `loopflowstudio` has graphite-app, xcode-cloud
  and railway-app installed. The laptop's `gh` token carries `repo`, `workflow`,
  `write:packages`, `gist`, `read:org`.

Passed over:

- **Installation token.** Tightest (per rental, 1 h, self-revoking) but every
  rental needs the laptop each hour: close the lid and a rental cannot push.
  It also needs a private key per person and turns PR authorship into a bot.
- **Fine-grained PAT.** No renewal problem, but it cannot be minted, so its
  expiry is days chosen by hand and it outlives every rental.
- **A device approval per rental** (what OpenCode OAuth does, `credentials.rs:101-120`).
  Independent tokens, but GitHub is required for every rental, so every
  placement would need a code typed or an approver command.

Tension left visible: the brief asks for "an expiry no longer than the rental".
GitHub offers 8 h for this token type and nothing shorter. The plan ends the
token early by refreshing the grant when the repository's last live rental is
given back from this laptop; a rental the guardian deletes while the laptop is
away leaves a token that dies at its 8 h mark.

A rental longer than the token's remaining life loses GitHub at that mark
unless the laptop renews it. `rental::DEFAULT_HOURS` is 4; `MAX_HOURS` is 24.

## Source of truth

`github_rental_grants`, one row per repository on the laptop: repository id
(GitHub's numeric id and `owner/name`), login, access token, refresh token,
`expires_at`. Encrypted with `token_crypto` like `provider_tokens`. Plus
`rentals.github_repository` (nullable): which repository's token a rental
holds, read to find who needs a re-send and when the last holder is gone.
One migration (`scripts/new_migration.py`), additive.

The client id is the personal setting `forward.github_app` (my choice of name:
it sits with `forward.env` and `forward.files`, what crosses to other
Machines; dropped from repository layers like the rest of `forward.*`,
`config/settings.rs:543-558`). No compiled default until a public Loopflow app
is registered.

## Behavior

- **Grant** (`lf machine login github --rental`, run in a checkout): device
  flow with `scope=offline_access`, exchange with the origin's `repository_id`,
  check `GET /user` equals the laptop's `gh` login, store the row. Uses the
  existing approver path (`login.rs` `send`/`Page`).
- **Token for a rental** (`credentials::connect`, when
  `sqlite.rental_for_machine` finds a rental): use the stored access token when
  it outlives the rental's deadline; otherwise refresh, store, and re-send to
  every other live rental recorded for that repository. A failed re-send names
  the rental and `lf machine login github <label>`.
- **Send:** `Credential::Github { login, token }` unchanged on the wire. The
  receiving `install` treats GitHub as installed only when `hosts.yml` holds
  that login with that exact token, so a renewed token replaces the old one
  (`credentials.rs:274-286,353-356`).
- **Renew by hand:** `lf machine login github <rental label>` runs the same
  path. Placement prints when the rental's deadline passes the token's expiry.
- **Give back** (`rental/recovery.rs` `give_back`, `delete_sandbox`): clear
  `rentals.github_repository`; when no live rental holds that repository,
  refresh the grant so the handed-out token stops working.
- **Owned Machines** (`lf machine login github <ssh box>`) keep receiving the
  laptop token: the person's own hardware, unchanged.
- **Empty `logins`:** `sign_ins` returns the template's list and nothing else;
  `prepare` already adds GitHub and Linear (`rental.rs:870-873`). The
  `local_machine`/`login::logins` branch (`rental.rs:841-849`) goes. A rental
  that should run Codex names it: `--logins codex`. The agent-login check at
  `rental.rs:697` already says so.

## Absent and error states

- GitHub origin, no `forward.github_app` or no grant for this repository:
  placement stops before anything is rented, beside the Claude-token check
  (`rental.rs:697-704`), naming the two commands. Nothing falls back to the
  laptop token.
- Refresh refused (grant revoked, 6 months passed): same message, grant row
  kept until a new device approval replaces it.
- Refreshed token's login differs from the laptop's: discarded, as Linear does
  (`credentials.rs:74-77`).
- App not installed on the repository: GitHub ignores `repository_id` rather
  than failing, so the grant step checks `GET /repos/<owner>/<name>` with the
  new token and that a second repository of the person's is not readable
  before storing.
- Non-GitHub origin: no GitHub sign-in, as today.

## Unanswered until a real app exists

No GitHub App can be registered headless, so these stay open and Pass 1 answers
them first:

1. Does a refreshed token keep the `repository_id` restriction? If not, the
   design changes to: no `offline_access`, a fresh device approval whenever the
   stored token cannot cover a new rental (about one per repository per
   half-day), and no re-send.
2. Do two device grants (two repositories) coexist for one person and app?
3. Does `gh` accept `ghu_` for `auth login --with-token`, `repo clone`,
   `pr create`, `pr merge --auto`, `pr checks`, and a push touching
   `.github/workflows/`?
4. Is the old access token dead at once after a refresh?

App registration the probe needs: device flow enabled, user tokens expiring,
no webhook, installed on the repository, repository permissions Contents
write, Pull requests write, Workflows write, Actions read, Checks read, Commit
statuses read, Metadata read.

## Security and privacy

Protected: the laptop's `gh` token and the refresh grant never leave the
laptop; credential bytes travel on SSH stdin only; `Credential` keeps no
`Debug`; the grant table is encrypted. `lf security-review` runs before
delivery: this changes credential forwarding to Modal, Daytona, RunPod and AWS
rentals, and all four share `prepare` and `attach`.

Folded in from finding 12 (`scratch/security-review.md` in the main checkout):

- `first_contact` (`rental.rs:460-475`) adds `-a -x`.
- `forget_host_key` is skipped for Daytona, whose route is the stable gateway
  `ssh.app.daytona.io`.
- One redaction of `ssh://user@` userinfo where routes reach error text
  (`machine/connection.rs:26-48`, `rental/recovery.rs:164,389`).
- `rental.rs:946` quotes the repository id with `q()`.

## Exclusions

- Locally generated host keys passed in create requests (rest of finding 12).
- Daytona's token in `ssh` argv and `~/.lf/config.yaml`.
- A second private repository on a rental (submodule, private Git dependency).
- Automatic renewal by the laptop's server for rentals past 8 h.
- Owned Machines' GitHub login.

## What goes

- `gh_token()` as the source of a rental's credential (`credentials.rs:54-64`
  keeps it for owned Machines only).
- The "every provider signed in here" branch of `sign_ins` and its sentence in
  `docs/ops.md:270-271` and `docs/subscriptions.md:163-166`.
- The `installed`-by-login shortcut for GitHub on receive.

Survives: Linear's renewed token, OpenCode and Codex sign-in when listed in
`logins`, `gh auth setup-git` on the rental, `gh repo clone` in
`checkout_script`.

## Passes

1. **Mint and prove.** Grant table and migration, `forward.github_app`,
   `lf machine login github --rental`, refresh, and the four probes above
   against a real app. Shows whether refresh keeps the restriction; if it does
   not, the plan switches to the no-refresh shape before anything else is built.
2. **Rentals use it.** `connect` branch for rentals, exact-token install,
   re-send on refresh, the pre-rent check, give-back refresh,
   `rentals.github_repository`, empty-`logins` change, docs
   (`docs/ops.md`, `docs/subscriptions.md`, `docs/security.md`).
3. **First contact.** `-a -x`, Daytona host key, userinfo redaction, `q()`.

## Acceptance

Headless, with GitHub's endpoints stood in the way `LF_TEST_MACHINE_GITHUB_URL`
already does (`credentials.rs:571-575`) and a fake `gh` whose `auth token`
prints a sentinel:

- The bytes `send` writes for a rental do not contain the sentinel, and the
  token in them is the stand-in's `ghu_` value.
- A token that expires before the rental's deadline triggers one refresh, and
  a second live rental of the same repository receives the new token.
- A GitHub origin with no grant fails before `admit_rental`.
- `sign_ins(&[])` is empty; a signed-in OpenCode here sends nothing.
- `first_contact` argv contains `-a` and `-x`; `forget_host_key` on a Daytona
  route runs no `ssh-keygen`; an error built from `ssh://SENTINEL@host` never
  contains `SENTINEL`.

Done when the demo above runs against a real rental.
