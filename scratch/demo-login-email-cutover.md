# Demo: login emails in the settings cutover (Task 3fcb3f)

Reviewed 2026-11-05, headless; an agent stood in for Jack Heart at the demo step. No person observed this.
Change under review: PR #1825, branch head `887a787efb`. Design: the Task brief (no separate design doc).

## Experience

A Machine still on a pre-cutover store upgrades to a build with the cutover, then launches an agent on its
saved account route.

Setup: `lf` 0.14.19 (retained binary) created a real store. Three Codex accounts were added by SQL, with ids
that are not prefixes of their emails: `personal` (side@example.com), `engineering` (work@example.com),
`local` (no email). Default route: personal, engineering, local. Stand-in Codex credentials, private
`HOME`/`LF_HOME`, inherited `LF_*` cleared. Identical copies were upgraded by each build.

## Observed

| | Unfixed, installed `5519340b4` | Branch `887a787ef` |
|---|---|---|
| Exported `account.codex` | `personal, engineering, local` | `side@example.com, work@example.com, local` |
| `lf account route --default` | exit 1: `account reference personal has no resident login` | exit 0: 1. personal · side@example.com, 2. engineering · work@example.com |
| `lf -b -a codex default 'say hi'` | fails: `failed to select provider account: account reference personal has no resident login`; provider never started | selects the route; `~/.codex/auth.json` holds side@example.com; starts the stand-in `codex app-server`, which then exits, as a stand-in does |

0.14.19 showed the same route order before upgrade, so the branch preserves it. Branch store: `provider_routes`
gone, 98 migrations applied. A repo-scoped route to an absent clone (earlier seed, same build) exported
`side@example.com` to `settings-recovery.yaml`.

An earlier seed used ids `work` and `side`: the unfixed build passed, because `match_account` also matches email
prefixes. Machines whose id happens to prefix the email never saw this bug; the unit test's `loopflow` id does not.

## Proof limits

- Stand-in Codex; no real provider inference, no Claude credential path.
- Source build through a disposable home; no installed release.
- The `local` id-only entry was exported and resolved without error, but it has no credential and is not a usable
  candidate (existing behavior: identity needs an expected email).

## Not covered by this change

A Machine already cut over by an unfixed build keeps ids and fails as shown. The Task scopes this to Machines that
haven't cut over; Jack's Mac was repaired by hand. Any other cut-over Machine needs the same edit to `account:`.

## Recommendation

Advance. Next proof: installed release on a Machine that hasn't cut over.

## Evidence

- Durable root (kept): `~/.lf/demos/3fcb3f-login-emails/durable` (`seed`, `unfixed`, `fixed`; `LF_HOME` is `<name>/lf`).
- Runtime root held copied binaries; removed after the run. No Sessions or owners survived; nothing to tear down.
