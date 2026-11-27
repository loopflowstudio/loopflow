# Questions

## Cannot be undone once shipped

- **Who owns the GitHub App, and what is it called?** Its name, owner and
  address show on every approval page and installation. The plan assumes one
  public "Loopflow" app under `loopflowstudio`, client id compiled into `lf`.
  The alternative is an app each person registers. Either way a person
  registers one on github.com before Pass 2 can finish; settings are in the
  plan under "Needs a person".
  Answered 2026-11-27, Jack: "Owner should either be Loopflow Studio or Jack
  Heart if it needs to be a person". An organization can own an app, so the
  owner is `loopflowstudio`. Registration itself: "go ahead and do it".
- **Its permissions.** Adding one later asks every installation to approve
  again; removing one asks nobody. The plan started without Workflows.
  Answered 2026-11-27, Jack: "i would add the workflow perm". The app gets
  Workflows write when it is registered.

## Made with nobody here; reversible before build

- **Token mechanism.** GitHub App user token by device flow, one grant per
  repository on the laptop, over an installation token (1 h, needs the laptop
  hourly, bot authorship) and a fine-grained PAT (cannot be minted, lasts
  days).
- **8 hours, not the rental's length.** GitHub has no shorter user token. A
  rental deleted while the laptop sleeps leaves its token working until the
  8 h mark or until the laptop next notices. The only tighter option is a code
  typed at every placement.
- **Approval happens inside `lf task run --rent`** at a repository's first
  rental from a terminal; with no terminal, placement stops before renting.
- **`lf ondemand login github`** is the name for approving ahead of time, over
  `lf machine login github --rental`.
- **A rental past 8 h** gets a printed stop time and a renew command; the
  laptop renewing by itself is a deferred finding.
- **Empty `logins` drops Codex.** A template with no `logins` used to sign a
  rental in to Codex when the laptop was; it now needs `--logins codex`.
- **`github` and `linear` in `--logins`.** First made: accepted, doing
  nothing, because rejecting them would break templates already saved with
  them. Changed after the review of the build (Pass 4): refused at
  `lf ondemand add`, dropped when a saved template is read.
- **Owned Machines still get the laptop token.**
- **File the two deferred findings as Tasks?** Automatic renewal; signing
  Codex and OpenCode out at give-back.

## Raised by writing the docs

- **Approve before installing the app, and the code is wasted.** Docs say
  install first; skipping it means a second code after installing. Can `lf`
  see the missing install before showing a code?
- **No terminal, but a command approver is set:** does placement approve
  through it (docs say yes) or stop (plan's absent states say stop)?
- **App address in the docs** is `https://github.com/apps/loopflow-studio`.
- **`lf ondemand login github` is in the docs before Clap has it**, so
  `documented_commands` fails until Pass 2.

## Raised by Pass 1

- **Owned Machines added as `ssh://jack@mini` lose the user in errors too.**
  The elision is by route shape, not by rental, so the suggested
  `ssh -- 'ssh://…@mini'` cannot be pasted. A plain `jack@mini` is untouched.
  Narrowing it to rentals needs the caller to say which a route is.
- **The no-agent error names `--logins codex` on `lf ondemand add <name>`.**
  There is no command that edits a saved template's logins; `add` again with
  the full flag set is the only way.
- **Land Pass 1 alone?** The first-contact and `logins` fixes are real without
  the app. Landing them alone means pulling the Pass 2 and 3 docs out of this
  branch and the token work taking a second Task.

## Raised 2026-11-27: what customers trust

- **Installing the shared app trusts its owner.** An admin of `loopflowstudio`
  can generate a private key for the app at any time and with it mint tokens
  for every repository any customer installed it on: Contents write and
  Workflows write, so code and, through a workflow, Actions secrets. Stated
  from how GitHub Apps work, not probed. Neither the plan nor
  `docs/security.md` says so yet.
- **Device codes can be phished.** The client id is public; anyone can start
  an approval and have a customer type the code on GitHub's real page under
  the real app name. The token reaches only repositories where that customer
  installed the app.
- **Passed over, for now.** Jack, after hearing both: "ok, i guess the trust
  us model is best then for now". The alternatives were nothing on the rental
  (the Machine forwards checked pushes with its own token, as Claude Code on
  the web does on Anthropic's side), a per-rental deploy key, a hand-made
  fine-grained token, or an app each person registers.

## Raised by Pass 2

- **What a token reaches is asked through `/user/installations`,** not by the
  plan's two repository reads. Unprobed: whether that listing, for a token
  restricted by `repository_id`, shows only the one repository. If it shows
  every repository of the installation, every approval fails with "reaches N
  more" and the check has to become the two reads.
- **A check that cannot reach GitHub after a renewal loses the grant.** The
  refresh token is spent by then and the new tokens are not stored until the
  check passes, so a dropped connection costs one more typed code. Storing
  first would keep the grant but leave an unchecked token in the store.
- **`lf ondemand login github` always asks for a new code,** also when the
  repository already has a working grant. Whether a second device approval
  ends the first one's tokens is probe 2.
- **`LF_APPROVE_MACHINE` is `rentals`** for this approval: no Machine exists
  yet when it is asked.

## Raised by Pass 3

- **Does `gh auth login --with-token` take a `ghu_` token?** `install` pipes
  it exactly as it pipes the laptop token. Probe 3 answers it; if `gh` refuses
  for missing scopes, the rental needs `GH_TOKEN` or a Git credential helper
  instead of `gh`'s file.
- **A rental's deadline past 8 h renews on every sign-in.** No token outlives
  such a rental, so each placement or `lf machine login github` of that
  repository renews and re-sends to its other rentals.
- **A failed renewal with a terminal goes straight to a new code,** without
  printing why the renewal failed.
- **`lf ondemand rent` by hand signs in to nothing on GitHub.** It has no
  repository; `lf machine login github <label>` from a checkout does it.
- **The ending's renewal can wait on GitHub** (15 s a request, four requests)
  inside `lf machine end` and the server's rental check.

## Raised by the review of the build (`scratch/concept-review.html`)

- **The command that approves ahead of time:** `lf ondemand login github`
  (built, documented) or `lf machine login github --rentals` (beside the
  other GitHub sign-ins, where `lf machine logins` sends people). The review
  recommends neither. Easy to change until this lands.
- **How long does a waiting placement wait?** Pass 4 makes the lock wait. The
  first rental of a repository holds it while a person types a code, so a
  second placement of that repository waits as long as the typing takes.
- **Recording what a rental was sent as the token's expiry** assumes two
  tokens of one repository never share an `expires_at`. A renewal within the
  same second would; a counter on the grant would not.
- **Does the ten-token limit cover App user tokens?** Probe 5 in the plan.
