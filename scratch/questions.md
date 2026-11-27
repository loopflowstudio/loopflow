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
  again; removing one asks nobody. The plan starts without Workflows, so a
  Task that edits `.github/workflows/` cannot push from a rental.

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
- **`github` and `linear` stay accepted in `--logins` and do nothing.**
  Rejecting them would break templates already saved with them.
- **Owned Machines still get the laptop token.**
- **File the two deferred findings as Tasks?** Automatic renewal; signing
  Codex and OpenCode out at give-back.

## Raised by writing the docs

- **Approve before installing the app, and the code is wasted.** Docs say
  install first; skipping it means a second code after installing. Can `lf`
  see the missing install before showing a code?
- **No terminal, but a command approver is set:** does placement approve
  through it (docs say yes) or stop (plan's absent states say stop)?
- **App address in the docs** is `https://github.com/apps/loopflow` until the
  app is registered.
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
