# LOO-1118: rentals receive a repository-scoped, expiring GitHub token

Passes 1 to 6 are built and tested against a stand-in GitHub. No further pass
is worth building before the run under "Needs a person now". Nothing has run
against GitHub's real app or a real rental. `scratch/concept-review.html`
(written at `06628a5477`) reviewed the build after Pass 6; where each finding
went is under "Review".

## Needs a person now

Nothing has been asked of GitHub with an app token: no install, no code typed.
Both steps act as a person on github.com, so no run here takes them. Checked
2026-11-27: `loopflowstudio` has graphite-app, xcode-cloud and railway-app
installed, not Loopflow Studio.

**Made for review:** the first run installs on two repositories, not one.
With one install `/user/installations` lists one repository whether or not
GitHub restricted the token, so the check (`reach`, `rental/github.rs`) passes
either way and the run would not show the restriction. Passed over: one
repository, which exercises approval and renewal only.

1. Install the app on two private repositories: open
   `https://github.com/apps/loopflow-studio`, Install, "Only select
   repositories", `loopflowstudio/loopflow` and one other private repository.
   Whether the install completes with no private key generated is itself
   unprobed.
2. From a terminal in a checkout of the first, with this branch's `lf`
   and a private `LF_HOME` (the migration is unmerged):
   `lf ondemand login github`, then type the printed code at
   `https://github.com/login/device` as the account `gh` is signed in to.
3. Read the second repository with the stored token: it must be refused.
   Renew once and repeat both (probe 1).

What each answer means:

- Approval passes and the second read is refused: the restriction holds and
  the check sees it. The build stands. If the renewed token is still
  restricted, the check of a renewed token can go (three requests a renewal;
  the second `reach` in `usable`).
- Approval prints `reaches 1 more than …; discarded`: the token may be
  restricted but the listing shows the whole installation. The check becomes
  a read of a second installed repository that must fail; with one install
  there is nothing to check.
- The second read succeeds: there is no repository-scoped user token and the
  mechanism is the wrong one. Pass 1, the approval flow and the record of
  what each rental was sent survive.
- A renewed token loses the restriction: the renewal, the re-send, the cut at
  the last ending and the lock go, about half of `rental/github.rs` (probe 1's
  fallback).

That run also answers whether GitHub issues a refresh token for
`scope=offline_access`. Probes 2 to 5 then need the stored grant and a rental
or any box with `gh`.

## Outcome

A rental never holds the laptop's `gh` token. It holds a GitHub App user
access token (`ghu_…`) that reaches the one repository the rental runs, acts
as the same person (PR author unchanged), and stops working within 8 hours of
issue, sooner when the repository's last rental ends. The grant that mints it
stays on the laptop, as Linear's does.

A person sees one approval per repository, at its first rental, and later an
expiry time. They never see an app registration, a client id or a grant.

A template with no `logins` signs the rental in to GitHub and Linear only.

Rental first contact stops forwarding the SSH agent and X11, Daytona's pinned
gateway key stays pinned, and a route's userinfo no longer appears in errors.

## Demo

```bash
# once per repository, on github.com: Install the Loopflow app on it
lf task run LOO-570 --rent code
#   Rentals need GitHub for loopflowstudio/loopflow.
#   Approve at https://github.com/login/device  code ABCD-1234
#   Renting 4 vCPU 8 GB on daytona …   GitHub until 07:40
ssh <rental> 'gh auth token | cut -c1-4'                    # ghu_
ssh <rental> 'gh api repos/<owner>/<other-private-repo>'    # 404
ssh <rental> 'cd ~/src/loopflow && git push --dry-run'      # works
lf machine logins <rental>                                  # github  signed in
```

## The app (registered 2026-11-27)

"Loopflow Studio", owned by `loopflowstudio`, public (any account can install
it). Client id `Iv23likXtOBTpX1WyV4f`, app id 5957089, address
`https://github.com/apps/loopflow-studio`. The name "Loopflow" was refused:
"Name is reserved for the account @loopflow". Device flow on, user tokens
expiring, no webhook, no private key generated. Repository permissions:
Contents write, Pull requests write, Actions read, Checks read, Commit
statuses read, Metadata read, Workflows write.
It is installed nowhere yet: the four probes need it installed on a
repository and a person to type the device code.

After registering, GitHub's page said "You must generate a private key in
order to install your GitHub App", yet its Install page offered Install
buttons with no key present. Whether install completes without a key is
unprobed.

## Mechanism, and what was passed over

**Made for review, nobody was here to ask:** a GitHub App user token by device
flow, one grant per repository kept on the laptop, its access token handed to
that repository's rentals; the app is Loopflow's, its client id compiled in.

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
- Before 2026-11-27 no Loopflow GitHub App existed; `loopflowstudio` has graphite-app, xcode-cloud
  and railway-app installed. The laptop's `gh` token carries `repo`, `workflow`,
  `write:packages`, `gist`, `read:org`.

Passed over:

- **Installation token.** Tightest (per rental, 1 h, self-revoking) but every
  rental needs the laptop each hour: close the lid and a rental cannot push.
  It also needs a private key per person and turns PR authorship into a bot.
- **Fine-grained PAT.** No renewal problem, but it cannot be minted, so its
  expiry is days chosen by hand and it outlives every rental.
- **A device approval per rental** (what OpenCode OAuth does, `credentials.rs:101-120`).
  One token per rental, but a code typed at every placement.
- **An app per person, its id in `forward.github_app`** (the kickoff plan).
  Every person learns registration and six permissions for no gain, and
  `forward.*` is what crosses to another Machine; a client id never does.
- **The client id from the environment or Doppler**, as `LINEAR_CLIENT_ID` is
  read: a secret's home for a value that is not secret.

Two tensions left visible:

- The brief asks for "an expiry no longer than the rental". GitHub offers 8 h
  for this token type and nothing shorter. The token is cut when the
  repository's last live rental ends on this laptop; a rental its provider
  deletes while the laptop sleeps leaves a token alive until its 8 h mark or
  until the laptop next notices.
- The token belongs to the repository's grant, not the rental. Placing a
  second rental can renew the grant, which kills the token the first one
  holds; the laptop re-sends, and when that SSH fails the first rental's push
  gets a 401 until `lf machine login github <label>`. A rental past the
  token's 8 h meets the same. `rental::DEFAULT_HOURS` is 4; `MAX_HOURS` is 24.

## Source of truth

`github_rental_grants`, one row per repository on the laptop: repository id
(GitHub's numeric id and `owner/name`), login, access token, refresh token,
`expires_at`. Encrypted with `token_crypto` like `provider_tokens`. Plus
`rentals.github_repository` (nullable): which repository's token a rental
holds, read to find who needs a re-send and when the last holder is gone. It
is stored because it cannot be derived: a rental from `lf ondemand rent` has
no Task, and `mark_ended` hands the Tasks back first. One migration
(`scripts/new_migration.py`), additive.

## Behavior

As planned through Pass 3; Pass 4 (under "Passes") replaced the approval
check, the re-send, the ending and `--logins` described here.

- **Approve** at the first placement of a repository from a terminal, inside
  `lf task run --rent`, before anything is rented, where `prepare` already
  runs Codex and OpenCode sign-ins: device flow with `scope=offline_access`,
  exchange with the origin's `repository_id`, check `GET /user` equals the
  laptop's `gh` login, check the token reads this repository and cannot read a
  second one of the person's, store the row. When the first read fails, print
  the app's install link and stop. Uses the existing approver path
  (`login.rs` `send`/`Page`).
- **Approve ahead of time:** `lf ondemand login github`, run in a checkout,
  the same step by itself (name made for review; passed over
  `lf machine login github --rental`, which signs no Machine in). A placement
  with no terminal and no grant stops before renting and names it.
- **Token for a rental** (`credentials::connect`, when
  `sqlite.rental_for_machine` finds a rental): use the stored access token when
  it outlives the rental's deadline; otherwise refresh, store, and re-send to
  every other live rental recorded for that repository. A failed re-send names
  the rental and `lf machine login github <label>`.
- **Send:** `Credential::Github { login, token }` unchanged on the wire. For a
  rental the sender skips `remote_has` (`credentials.rs:57-59`, which compares
  names) and sends its current token every time; the receiving `install`
  replaces a different token and keeps an equal one
  (`credentials.rs:272-286,353-356`).
- **Signed in means the token works:** a rental's GitHub row in
  `lf machine logins` asks GitHub with the token the rental holds
  (`login.rs:345-350` reads a name from `hosts.yml` today), so a dead token
  reads signed out.
- **Renew by hand:** `lf machine login github <rental label>` runs the same
  path for the rental's recorded repository, from any directory (Pass 6).
- **Placement prints** the time GitHub stops on that rental.
- **Ending** (`rental/recovery.rs` `mark_ended`, the one place `give_back`,
  `delete_sandbox` and `refresh` all reach): clear
  `rentals.github_repository`; when no live rental holds that repository,
  refresh the grant so the handed-out token stops working. A failed call to
  GitHub is logged and never fails the ending.
- **Workflows write**, so any Task can run on a rental, including one that
  edits `.github/workflows/`. What it costs: a rental can push a branch
  carrying a workflow that reads the repository's Actions secrets.
- **Owned Machines** (`lf machine login github <ssh box>`) keep receiving the
  laptop token: the person's own hardware, unchanged.
- **Empty `logins`:** `sign_ins` returns the template's list and nothing else;
  `prepare` already adds GitHub and Linear (`rental.rs:870-873`). The
  `local_machine`/`login::logins` branch (`rental.rs:841-849`) goes. The
  no-agent error (`rental.rs:697-704`) says `lf machine login codex` here
  today; it names `--logins codex` on the template instead. `github` and
  `linear` in `--logins` no longer change anything, and the docs example
  `--logins codex,github` becomes `--logins codex`.

## Absent and error states

- GitHub origin, no grant, no terminal: placement stops before anything is
  rented, beside the Claude-token check, naming `lf ondemand login github`.
  Nothing falls back to the laptop token.
- Refresh refused (grant revoked, 6 months passed): a new approval, by the
  same rule. The grant row is kept until the approval replaces it.
- Refreshed token's login differs from the laptop's: discarded, as Linear does
  (`credentials.rs:74-77`).
- App not installed on the repository: GitHub ignores `repository_id` rather
  than failing, hence the check of what the token reaches (Pass 2).
- Non-GitHub origin: no GitHub sign-in, as today.

## Unanswered until the app exists

The run under "Needs a person now" answers the first:

1. Does a refreshed token keep the `repository_id` restriction? If not, the
   design changes to: no `offline_access`, a fresh device approval whenever the
   stored token cannot cover a new rental (about one per repository per
   half-day), and no re-send.
2. Do two device grants (two repositories) coexist for one person and app?
3. Does `gh` accept `ghu_` for `auth login --with-token`, `repo clone`,
   `pr create`, `pr merge --auto`, `pr checks`? Does a push touching
   `.github/workflows/` go through?
4. Is the old access token dead at once after a refresh?
5. Is there a cap on grants per person and app? GitHub's "Token revoked due
   to excess of tokens for an OAuth app with the same scope" says: "There is
   a limit of ten tokens that are issued per user/application/scope
   combination, and a rate limit of ten tokens created per hour." The pages
   (`token-expiration-and-revocation`, `refreshing-user-access-tokens`,
   `generating-a-user-access-token-for-a-github-app`, read 2026-11-27) do not
   say whether it covers a GitHub App's `ghu_` tokens, which carry no scopes.
   If it does, an eleventh approved repository ends the first one's grant
   without a word, and a rental past 8 h, which renews on every sign-in, can
   meet the hourly limit. Probe: approve an eleventh repository, or renew
   eleven times in an hour. The cut at a repository's last ending is itself
   a renewal, so Tasks run on rentals one after another renew once or twice
   each: under an hourly limit of ten, a handful of back-to-back rentals an
   hour meets it with no long rental involved.

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
- Owned Machines' GitHub login.

## What goes

- `gh_token()` as the source of a rental's credential (`credentials.rs:54-64`
  keeps it for owned Machines only).
- The "every provider signed in here" branch of `sign_ins` and its sentence in
  `docs/ops.md:270-271` and `docs/subscriptions.md:163-166`.
- For rentals: the `remote_has` early return on send, the `installed`-by-login
  shortcut on receive, and the name-from-`hosts.yml` GitHub row.

Survives: Linear's renewed token, OpenCode and Codex sign-in when listed in
`logins`, `gh auth setup-git` on the rental, `gh repo clone` in
`checkout_script`.

## Passes

1. **What needs no app.** Built at `efaa6313d3`: first contact `-a -x`, Daytona
   host key kept, `ssh://user@` elided in `connection_failure`, `q()` on the
   repository id, empty `logins` signs in to no agent. Nothing learned about
   the token. The docs for Passes 2 and 3 are already written, so
   `docs/subscriptions.md`, `docs/ops.md` and `docs/security.md` describe a
   GitHub token rentals do not hold yet. This branch does not land before Pass 3.
2. **Mint, unproven.** Built at `adc26b51db`: `github_rental_grants`, the compiled
   client id, `lf ondemand login github`, renewal
   (`rust/loopflow-execution/src/rental/github.rs`), `docs/security.md` on
   what installing the app trusts. Tested against a stand-in only. Changed
   from the plan: instead of reading this repository and one other, every
   mint and renewal asks `GET /user/installations` and each
   `/user/installations/<id>/repositories` what the token reaches and keeps
   it only when that is exactly the one repository. A read of the repository
   passes for any token when it is public, so it could not show a missing
   install. This also makes probe 1 safe before it is answered: a renewed
   token that lost its restriction is discarded, not handed out. Not built:
   the probes. `rentals.github_repository` joins the migration in Pass 3.
   Findings for Pass 3: `renew` takes the grant it read, so two `lf` renewing
   one repository at once race for the refresh token and the loser is told
   to approve again; Pass 3 takes `credentials::credential_lock("github")`
   around read-then-renew. `credentials::github_login` and
   `LF_TEST_MACHINE_GITHUB_URL` are gone; `rental::github::login_of` and the
   `TEST_GITHUB_URL` task-local replace them.
3. **Rentals use it, unproven.** Built at `dfbd8864c9`: approval before `rent` in
   `place` (`github::approve_first_rental`), `credentials::connect_rental`
   for every rental GitHub sign-in (`prepare` passes the Task's repository;
   `lf machine login github <rental>` passes the directory it runs in),
   re-send after a renewal, replace-on-different in `install`, the cut in
   `mark_ended`, `rentals.github_repository_id`, `GitHub on <label> until
   HH:MM`. Changed from the plan:
   - The column is `github_repository_id` and is never cleared: holders are
     the live rentals carrying it, so an ended rental stops counting by its
     state.
   - The GitHub row asks GitHub on any Machine whose `gh` token starts
     `ghu_`, not "on a rental": a Machine cannot tell it is rented, and only
     a token that expires needs asking. No answer reads `unknown`.
   - An ending skips the renewal when the stored token already expired.
   - The renewal, the holder record and the retirement all run under
     `credential_lock("github")`, which fails at once when held: a second
     `lf` signing a rental in to GitHub at the same moment is told "another
     machine login is in progress", and an ending that meets it leaves the
     token to expire.
   Not covered by a test: the order in `place` (approval before
   `admit_rental`) is read from the code; the stop itself is
   `no_terminal_and_no_command_approver_asks_github_for_nothing`. No
   placement was run against a real rental.
4. **A rental's access is a record.** Built at `b12b2c59e3`, made for review:
   `rentals.github_token_expires_at`; one step `github::settle` called by
   placement (`settle_before_renting`, before `rent`), `github::sign_in`
   (the one door, reached through `login::login`'s new `repo` argument),
   endings and a rental moved to another repository (`settle_unattended`);
   a per-repository lock that waits; the install link above every code;
   `--logins github|linear` refused at `add` and dropped on read.
   `RentalToken`, `connect_rental`, `RentalGithub`, `approve_first_rental`
   and `retire_token` are gone. What it taught: one step does carry all four
   callers with no flag between them, so the deferred automatic renewal is
   `settle_unattended(repository_id)` from the server's rental visit.
   Changed from the plan:
   - **A token with 7 hours left counts as new** (`AS_GOOD_AS_NEW`). Without
     it a rental past 8 h makes every settle renew, endings included, and a
     placement would renew twice (before and after renting).
   - **A sign-in clears what the rental was recorded as sent,** so
     `lf machine login github <rental>` always sends; a settle from any other
     caller skips rentals recorded as current.
   - **Placement asks for 10 minutes past the quoted deadline**
     (`ADMISSION_MARGIN`): the rental's own deadline is set at admission,
     moments later.
   - **`lf ondemand login github` sends the new token** to that repository's
     running rentals, since a new approval makes them behind.
   - The lock is taken with `spawn_blocking`; an ending waits behind a person
     typing a code (up to the code's 15 minutes), the server's rental check
     included.
   Not covered by a test: the order in `place` (settle before `rent`) is read
   from the code; the install link is printed, not asserted; SSH itself is
   stood in by `credentials::TEST_SENDS`.
5. **The record is read, and the approval is asked once.** Built at `99fd9808dc`,
   made for review: `lf ondemand list` shows `GitHub until HH:MM` or
   `GitHub not delivered` per live rental (`rental_github_delivery`, text
   only, not in `--json`); `lf ondemand login github` is
   `settle(until = now)` (`approve_ahead`); a settle with an asker asks
   GitHub what a stored token reaches (`standing`); the placement line names
   the renewal when the token ends first; a held lock prints one waiting
   line. `approve` is gone. Changed from the plan:
   - **An approval GitHub still honors is kept, not renewed,** by
     `lf ondemand login github`: the three reach requests answer "does it
     work", and a renewal would end what running rentals hold and count
     against the token limit of probe 5. It renews only an expired or
     rejected (401) token.
   - **The check runs for every settle someone is present for,** so
     `lf machine login github <rental>` and the sign-in after renting ask it
     too: a placement asks twice, six requests. One rule, no flag.
   - **A token that works but lost the repository is not renewed:** a
     renewal reaches what the approval reaches. With a terminal it goes to
     the install link and a new code; without, it stops with the install
     link and the grant is kept.
   - The waiting line is `Waiting for another lf to finish with GitHub for
     <owner/name>; if it shows a code, approve it there`: the lock's holder
     may be a renewal, not an approval.
   Not covered by a test: the list's text (run by hand against a seeded
   private store: `GitHub until 17:30`, `GitHub not delivered`, nothing for a
   rental with no repository; `--json` unchanged); the waiting line is
   printed, not asserted; `approve_ahead` itself reads the real `gh`, so the
   test drives `settle` as it does.
6. **A rental's repository is set once, and a dropped connection keeps the
   approval.** Built at `2d0c806405`, made for review. `sign_in_as` uses the
   rental's recorded repository and never reads the directory's origin for
   it; the move, its line, its settle and its test are gone. `renew` stores
   what GitHub issued and asks nothing; `usable` is the one place a token is
   checked (`Standing` and `grant_from` are gone). The settle step did get
   smaller. Changed from the plan:
   - **Every settle that sends a token asks GitHub what it reaches first,**
     with or without someone present. That closes the open point (an ending's
     re-send of a stored, unchecked token) with no "unchecked" column: three
     more requests on an ending that leaves other rentals of the repository
     running, and such an ending re-sends nothing while GitHub is unreachable.
   - **The cut at the last ending asks nothing of the new token,** one request
     where there were four: no rental is sent it, and the next settle checks
     it.
   - **A renewed token that reaches more stays stored and is never sent.**
     Each later settle asks again and gets the same refusal, or a new code
     when someone is present.
   - **A recorded repository with no grant does not count as set:** the first
     sign-in records before it approves, so a failed approval leaves a record
     that never had a token, and the directory is read again.
   - **No answer from GitHub no longer leads to a new code.** Only GitHub's
     refusal does; a failed request stops the command.
   Not covered by a test: an approval (not a renewal) whose check gets no
   answer still loses its code, as before.

## Acceptance

Headless, with GitHub's endpoints stood in the way `LF_TEST_MACHINE_GITHUB_URL`
already does (`credentials.rs:571-575`) and a fake `gh` whose `auth token`
prints a sentinel:

- The bytes `send` writes for a rental do not contain the sentinel, and the
  token in them is the stand-in's `ghu_` value.
- A token that expires before the rental's deadline triggers one refresh, and
  a second live rental of the same repository receives the new token.
- `lf machine login github <rental>` on a rental already holding that login
  sends the current token; the rental ends up with it.
- A rental holding a token GitHub refuses shows GitHub signed out in
  `lf machine logins`.
- A GitHub origin with no grant and no terminal fails before `admit_rental`,
  naming `lf ondemand login github`.
- A rental that `refresh` finds deleted by its provider refreshes the grant
  when it was the repository's last; with the GitHub stand-in failing, the
  rental still ends.
- `sign_ins(&[])` is empty; a signed-in OpenCode here sends nothing; the
  no-agent error names `--logins codex`.
- `first_contact` argv contains `-a` and `-x`; `forget_host_key` on a Daytona
  route runs no `ssh-keygen`; an error built from `ssh://SENTINEL@host` never
  contains `SENTINEL`.

Done when the demo above runs against a real rental.

## Docs

Written ahead of the build, describing intended behavior:

- `docs/subscriptions.md` "Rentals": the example and its output, the app
  install, what the token reaches, `lf ondemand login github`,
  `lf machine login github <rental>`, the stops, `--logins`.
- `docs/ops.md` "Rental credentials" bullet.
- `docs/security.md` "Credentials over SSH": a Rentals bullet, and first
  contact forwarding neither agent nor X11.

Questions the writing raised (also in `scratch/questions.md`):

1. **Approving before the app is installed costs a second code.** The device
   code is spent on a token GitHub did not restrict, and refresh takes no
   `repository_id`. The docs put the install first and say the next placement
   asks for a new code. Can the install be checked before the code is shown?
   (`GET /user/installations` needs an app user token; the laptop's `gh`
   token cannot ask.)
2. **Does a command approver let a placement with no terminal approve?** The
   docs say yes, as `approvers` already promises for every sign-in; "Absent
   and error states" above says such a placement stops.
3. **The app's address** is `https://github.com/apps/loopflow-studio`; the
   docs link it.
4. **Sample output wording** (`Rentals need GitHub for …`, `GitHub on code-1
   until 07:40`) is made for the docs; the build matches it or the docs
   follow.

## Review

What happened to each finding of `scratch/concept-review.html` (the review of
the build after Pass 6, written at `06628a5477`). Every adoption is made for
review; none is settled. Earlier reviews' findings were built in Passes 4 to
6.

- Giving a rental back could wait up to 15 minutes on another `lf`'s untyped
  code, inside the server's one recovery pass: fixed here. A settle with
  nobody to ask (`settle_unattended`, every ending, `lf machine end`
  included) tries the repository's lock once and does nothing when it is
  held (`repository_lock(…, wait)`, `rental/github.rs`). Passed over:
  waiting, as built in Pass 4. What it costs: when the holder was only
  confirming an approval and this rental was the repository's last, the
  token is not cut and stops at its own 8 hours; a rental left behind waits
  for the next command on the repository. Test:
  `an_ending_does_not_wait_behind_another_lf_settling_its_repository`.
  `docs/subscriptions.md` says so.
- A new code after a refused approval gave no reason: fixed here. `settle`
  prints GitHub's refusal above the code. The sentence ends "run
  `lf ondemand login github` in its checkout to approve again" with the code
  right below it; printed, not asserted.
- An ending with the lock free still asks GitHub (up to three requests) and
  SSHes to each rental that is behind, inside the recovery pass, each
  bounded by its timeout: left as built; it moves with automatic settling
  (deferred below).
- What rests on GitHub's answers (restriction, renewal keeping it, `gh`
  taking `ghu_`, the old token dying, the token cap): the probes under
  "Needs a person now" and "Unanswered until the app exists".
- One shared token per repository, the cut at the last ending, six requests
  a placement, a first approval that loses its code when its check gets no
  answer, `lf ondemand login github` keeping a working approval, a
  hand-rented box staying with its first repository, a person's command
  waiting behind another `lf`: left as built, as the review left them.
- The name `lf ondemand login github`, GitHub state in `--json`, filing the
  deferred findings: a person's choices, in `scratch/questions.md`.
- Automatic settling, Codex and OpenCode outliving the rental: deferred
  below.

## Deferred findings

- **The laptop renews a rental's GitHub token by itself.** Its server already
  visits each rental to collect Session history; renewing a token near its end
  on that visit removes the 8 h clock while the laptop is awake. After Pass 4
  it is one call to settle a repository from that loop
  (`settle_unattended`). It needs its own rule for when to renew: under the
  7-hour rule a long rental would be renewed on every hourly visit. It is
  also what delivers a missed token with nobody acting; until then
  "not delivered" waits for the next command on that repository. It needs
  its own place to run, outside the recovery pass, and would take over the
  re-send an ending does there today.
- **Codex and OpenCode outlive the rental.** Codex's device login is a
  renewing grant written on the rental; OpenCode gets a renewing login or a
  copied raw key; nothing signs either out before the sandbox is deleted.
  Signing both out in the give-back path helps only when the laptop does the
  giving back. Claude's setup token is long-lived, reaches the whole account,
  and has no narrower form.
