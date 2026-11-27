# Questions

Choices made with nobody here; each is reversible before build.

- **Token mechanism.** GitHub App user token by device flow, one grant per
  repository on the laptop, over an installation token (1 h, needs the laptop
  hourly, bot authorship) and a fine-grained PAT (cannot be minted, lasts
  days). The brief names the other two.
- **8 hours, not the rental's length.** GitHub has no shorter user token. The
  plan cuts it short at the last give-back. Is a token that can outlive a
  guardian-deleted rental by a few hours good enough?
- **Who registers the app.** The plan reads a client id from
  `forward.github_app`. A public "Loopflow" app with its client id compiled in
  would spare each person the registration; none exists.
- **Needed from a person before Pass 1 can finish:** a GitHub App with device
  flow enabled, installed on a repository, and its client id. Settings are
  listed in the plan under "Unanswered until a real app exists".
- **Empty `logins` drops Codex.** A template with no `logins` used to sign a
  rental in to Codex when the laptop was; it now needs `--logins codex`.
- **Owned Machines still get the laptop token.**
- **Setting name `forward.github_app`** and **flag `--rental`** on
  `lf machine login github`: both invented here.
