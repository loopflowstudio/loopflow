# Roster Tidy Review - 2026-07-06

## Recommendation

**Revise before landing.**

Land the Concerto `pm.asana_project` mapping. Deleting Mobile's local surface is
consistent with its archived goal. Do not land the Root and Workflows deletions
as-is unless the branch also records where their ownership moved and updates the
roster truth accordingly.

## Evidence

The roster-tidy commit itself is small: `209d88700 wave: link concerto to Asana,
retire root/mobile/workflows` changes seven wave files, adding three lines to
`wave/concerto/GOAL.md` and deleting `wave/mobile/GOAL.md`,
`wave/root/GOAL.md`, `wave/root/README.md`, `wave/root/backlog.md`,
`wave/root/root.yaml`, and `wave/workflows/GOAL.md`.

The full branch diff from this worktree is not only roster-tidy. It also carries
the `v0.10.0` release commit, version bumps, release notes, and release archive
movement. That should not be reviewed or landed as part of the roster decision;
rebase/cherry-pick the roster change onto the intended base first.

Current Root docs still define Root as the conductor wave. `wave/root/README.md`
lists Root plus Systems, Architecture, Concerto, Website, Workflows, and Goals as
the active roster, with Mobile explicitly archived. `wave/root/GOAL.md` says Root
owns garden passes, cross-wave legibility, status language, and scope hygiene.
Deleting Root removes that authored surface without replacing the conductor role.

Current Workflows still owns real engine responsibilities. `wave/workflows/GOAL.md`
assigns scheduling, providers, flow execution, mutation, and governance surfaces
to Workflows. Deleting it changes ownership of live engine work; the branch does
not move those responsibilities into Goals, Meta, Architecture, or Systems.

Current Mobile is different. `wave/mobile/GOAL.md` says the wave is archived,
has no active surface, and should not invent work. Removing that local surface is
compatible with the current goal and with the garden assessment's "silent" state.

The garden assessment supports resolving the roster, not blindly deleting all
limbo surfaces. It calls Concerto's Asana mapping necessary PM plumbing, marks
Mobile deletion consistent with archive direction, but marks Root and Workflows
as drifting because the current local files and roster-tidy disagree. It also
says Workflows deletion changes ownership of scheduling/provider/governance work
and needs a clear source of truth.

## Asana-Only Implications

Asana-only cuts both ways. The README says the roadmap lives in Asana and each
wave is pinned through `wave/<name>/GOAL.md` frontmatter. `lf op pm` reads
`pm.asana_project` from `GOAL.md`; waves without that field are skipped by PM
status and cannot run `pm show/update` without initialization.

That makes the Concerto change correct: Concerto already has an Asana mapping in
`wave/concerto/concerto.yaml`, but the PM code reads `GOAL.md`, so adding
`pm.asana_project: '1214270017631632'` to `wave/concerto/GOAL.md` makes Concerto
authoritative under the current runtime.

Root and Workflows currently lack `pm.asana_project` in `GOAL.md`, so they are
already outside the Asana-backed PM roster. The right fix is not necessarily to
keep stale local waves forever, but the branch needs one of these explicit
migrations:

1. Keep Root/Workflows as authored waves and add their Asana project mappings.
2. Retire them and move their responsibilities into named surviving waves
   (`meta`, `goals`, `systems`, etc.) with updated GOAL/README language.
3. Mark them as intentionally archived local docs, if they must remain readable
   but no longer PM-backed.

Without that migration, deleting the files makes Concerto's authored-wave list
and the local garden loop quieter, but also hides unresolved ownership.

## Verdict By Change

- **Concerto Asana mapping**: land.
- **Mobile local surface deletion**: land or keep only as archived documentation;
  either is consistent, but deletion is acceptable.
- **Root local surface deletion**: revise; do not land until conductor ownership
  moves somewhere explicit.
- **Workflows local surface deletion**: revise; do not land until engine,
  provider, scheduling, mutation, and governance ownership moves somewhere
  explicit.
- **Release/version changes on the branch**: keep out of the roster landing path.
