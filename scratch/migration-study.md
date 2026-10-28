# Migration study: app/CLI skew, dev builds on real data, staged contracts

LOO-656, 2026-10-28. Extends `scratch/migration-research.md` (Codex, OpenCode, Zed,
Atuin, Chromium, git/hg), which isn't repeated here. Each claim below was
confirmed 3–0 by adversarial verification unless marked. LOO-669's
`scratch/release-research.md` covers release streams; this note covers storage.

**Not covered.** No claims survived for VS Code/Cursor `state.vscdb`,
Electron apps in general, or Rails/Django/Ecto/Prisma id schemes and merge-order
conflicts. Those questions are still open; this note doesn't fill them from memory.

## 1. Components on different cadences against one interface

**Docker** negotiates. The client "negotiates the highest version of the API
supported by both the client and daemon … downgrading to an older version of the
API if necessary". Features from later versions are disabled, and compatibility
is "best effort". The daemon enforces a floor that moves over time:

- Engine 25.0–28.5 accepted API ≥ 1.24.
- 29.0–29.2 required ≥ 1.44.
- 29.3–29.8 went back to ≥ 1.40, after the jump broke clients still in the field.

`DOCKER_API_VERSION` turns negotiation off.
([Engine API reference](https://docs.docker.com/reference/api/engine/))

Docker retires old API versions in stages: deprecated and disabled by default in
v25, where `DOCKER_MIN_API_VERSION` re-enables them, then removed in v26. The
error names the fix: "client version 1.23 is too old. Minimum supported API
version is 1.24".
([docker/cli deprecated.md](https://github.com/docker/cli/blob/master/docs/deprecated.md))

**Tailscale** (macOS/iOS) only warns when the GUI and the network extension
differ in version. There's no refusal or negotiation. Medium confidence, single
source. ([v1.66.0 notes](https://github.com/tailscale/tailscale/releases/tag/v1.66.0))

**1Password** documents no minimum app or CLI version. The troubleshooting
answer to a mismatch is "use the latest desktop app". The claim that its CLI
authenticates through the app as a shared broker was refuted 1–2.
([app integration](https://www.1password.dev/cli/app-integration))

## 2. Dev builds on real data, and downgrades

**Mozilla separates by install.** Firefox ≥ 67 and Thunderbird ≥ 68 give every
installation (Nightly, Beta, Developer Edition, ESR) its own profile by default.
Profiles are assigned per install directory; the Profile Manager can still share
one. ([dedicated profiles](https://support.mozilla.org/gu-IN/kb/dedicated-profiles-firefox-installation))

Firefox detects downgrades. A profile last used by a newer version triggers a
warning that the older build can corrupt bookmarks and history, plus an offer to
create a new profile. The check reads `compatibility.ini`; `--allow-downgrade`
bypasses it.
([older-version profile](https://support.mozilla.org/en-US/kb/unable-launch-older-version-profile))
Thunderbird's 68.5 password storage and 78 address-book changes can't be undone
on downgrade, so a separate profile per install is only a default and doesn't
guarantee a downgrade is safe.

**Places replays migrations idempotently.** A downgrade only lowers `user_version`
and leaves newer objects in place, so the next upgrade re-runs the migrations over
them. A test enforces it: it lowers a current DB to
`FIRST_UPGRADABLE_SCHEMA_VERSION` and requires every migration to pass again
([test_current_from_downgraded.js](https://searchfox.org/firefox-main/source/toolkit/components/places/tests/migration/test_current_from_downgraded.js)).
The rule came from a 2014 incident. content-prefs' migrator treated a newer
schema as corruption: "no migrator function from version 4 to version 3;
backing up and recreating". Users lost per-site zoom levels. Builds older than 34
were left unfixed.
([bug 1074817](https://bugzilla.mozilla.org/show_bug.cgi?id=1074817))

**Signal refuses.** Before any migration runs, a `user_version` above the newest
migration it knows throws `DBVersionFromFutureError`. The startup dialog then
offers only "copy error and quit" and hides "delete and restart", because the
user "would almost never want to delete their data".
([migrations/index.node.ts](https://github.com/signalapp/Signal-Desktop/blob/17502b9d1a9e34af689f31508c4ac4ccee5b5cb7/ts/sql/migrations/index.node.ts),
[main.main.ts](https://github.com/signalapp/Signal-Desktop/blob/17502b9d1a9e34af689f31508c4ac4ccee5b5cb7/app/main.main.ts))

## 3. Staged destructive changes

**GitLab drops a column across three releases:**

1. Release M: `ignore_column` in application code.
2. Release M+1: drop it in a post-deployment migration (`db/post_migrate`, which runs after new code deploys).
3. Release M+2: remove the ignore.

Ignore and drop never ship in the same release. The wait is data in the code,
`ignore_column :updated_at, remove_with: '12.7', remove_after: '2019-12-22'`,
and the cleanup can merge only once both have passed.
([avoiding downtime](https://docs.gitlab.com/development/database/avoiding_downtime_in_migrations),
[post-deployment migrations](https://docs.gitlab.com/development/database/post_deployment_migrations))

**Required upgrade stops** land on X.2, X.5, X.8 and X.11 from about 17.5. Long
background migrations finalize in the first minor after the next stop, because
Omnibus times out after an hour. A migration may be removed or squashed only
after it has shipped in a stop, so users who skip versions can't miss it.
([required stops](https://docs.gitlab.com/development/database/required_stops),
[upgrade paths](https://docs.gitlab.com/update/upgrade_paths/))

## Patterns for Loopflow

These are synthesis, not sourced claims.

**How Desktop follows main (1a / 1b / 1c).** Docker shows what 1b and 1c would
need: an explicit supported range at the app↔CLI boundary, negotiation down
within it, and an error that names which side to update. Loopflow's DTOs have no
defaults and no versioned API, so that range doesn't exist today. 1b would have
to build it first. 1a is the lockstep case: Tailscale and 1Password don't
negotiate either, and get by with "keep them the same version". A recorded
build pair (CLI commit = app commit) is the cheapest correct answer, which
supports 1a. The store boundary still needs Docker-style range thinking either
way, because other Machines and streams read the same data at different versions.

**Refuse, never offer reset.** Signal and content-prefs mark the line. Released
lf ≤ 0.15.0, meeting a slug row, prints "incompatible Loopflow database; delete
loopflow.db". That's content-prefs' mistake as advice text. Every refusal should
name the build to install (Docker's "minimum supported is X") and never suggest
deletion. Membership floors already give Places-style tolerance for additive
migrations and Signal-style refusal for breaking ones; keep that split.

**Whether the schedule follows a channel (2).** Mozilla separates dev data by
default. OpenCode reverted the same separation this week because data "looked
gone" (see the earlier note). Jack accepted the shared store for main builds
("I think it can. I'm okay with that."), so the tension is already resolved for
LOO-656. Separation stays for unmerged schema work (`LF_HOME`). Firefox's
`compatibility.ini` is the piece to borrow: a schedule moving a Machine *back*
from main to stable must check the store first. That's the predicate LOO-669
calls before switching.

**Enforcing expand → wait 30 days → contract.** GitLab's `remove_after` makes the
wait part of the code. Loopflow can do one better, because `build.rs` already
reads main's first-parent history for order, which also gives every migration's
merge date. Sketch:

```sql
-- contract: add_wave_colour   (the expand it finishes)
-- breaks: drops waves.colour_legacy
```

`build.rs` refuses a breaking slug whose named expand merged less than
`CONTRACT_WAIT` (30 days, one constant) before the contract's merge, or before
now for unmerged builds. The wait comes from git, not from a remembered policy.
It also has to exceed stable lag (LOO-669: 7-day soak plus ≤ 1 day cadence),
so no stream gets stranded. Emergency breaks need an explicit override with a
reason. That's GitLab's skip and Docker's `DOCKER_MIN_API_VERSION`.

**Required stops (later).** Loopflow doesn't squash history today. If it ever
deletes old released batches, GitLab's rule applies: only after they've shipped
in a stop, with a refusal that names the stop release.

## Still open

- Rails/Django/Ecto/Prisma under concurrent branches. The existing note's
  OpenCode/Atuin timestamp sets are the closest evidence.
- VS Code/Cursor `state.vscdb` across Insiders and Stable.
