# Project configuration review — 2026-10-04

Design: [Keep every Wave ready for work](keep-every-wave-ready-for.md).

## Accepted direction

Jack Heart requested an explicit Wave configuration field for its Project,
suggesting `wave/<wave>/config.yaml`. Jack wants the core API to stay simple:
validate the configured Project and access, create a Project if none is configured,
and avoid name matching or candidate-selection machinery. This supersedes the
previous proposed status-based discovery and sole-candidate activation rules.
Existing Projects can be selected by configuring their exact UUIDs.

Ordinary Projects remain independent of chapters and default Flows. Optional
coordinated rotation preserves started work and changes the configured reference.
Jack also requested higher-level creation of a chapter Project for a particular
Wave and raised historical cross-Wave chapter inspection for design. No new
implementation or configured provider proof has occurred in this review.

## Proposed mechanics and remaining choice

- Field: `pm.linear_project` in the Wave's `config.yaml`; no duplicate field in
  `GOAL.md` frontmatter. Existing Wave config currently lives in that frontmatter;
  moving all other settings is not selected here.
- Ensure validates the exact configured ID or creates and records one. Preserve
  a reserved ID across lost responses and config-write failures to avoid duplicates.
- Reads remain observational and resolve the same configured ID. Opening never
  applies reset transfers or completes predecessor Projects.
- New Projects use the Wave name and no default Flow. Existing names/content stay.
- Before implementation, resolve which owning checkout writes config and how
  binding changes become visible across checkouts. An old config revision must
  not silently undo a reset; opening must not silently push repository changes.
- Configured proof still requires designated fixture Waves and write authority.

## Evidence and next work

`rust/loopflow/src/work/wave/config.rs` reads policy from `GOAL.md` frontmatter;
`ops/chapter.rs::select_current` currently selects by status. Both require changes
for config ownership. `apply_rotation` activates a successor before Task transfer;
recovery must preserve identities across provider effects and the config switch.
Desktop polls status, so it must share the configured-ID reader with ensure.

Next: finish the configuration-write ownership detail, then implement the shared
reader/ensure and explicit reset path with preservation proofs. This review
records no Flow navigation decision.

## Chapter history proposal

Keep chapter rotation above the core ensure API. Proposed optional Project
metadata carries a shared chapter key and display name; the key supports listing
old Projects across Waves independently of Project names and current config.
Exact provider storage and history projection remain open. Wave-scoped rotation
is requested; the historical inspection surface remains a proposed extension.
Do not introduce a Chapter table solely to perform this lookup.

## KR boundary — Jack Heart, 2026-10-04

Jack selected KRs as a prerequisite for creating Projects through the chapter
system. A review/planning skill produces per-Wave KRs and may also produce Task
candidates. Creating those Tasks is a separate step, optionally run globally
immediately afterward. Complete future Task planning must not block synchronous
chapter Project creation. Ordinary ensure remains usable without KRs.

The design now separates review/planning, chapter creation with supplied KRs,
and Task admission. Existing work preservation stays within rotation. In response
to Jack's concern about wiping old Tasks, the revised proposal removes automatic
backlog expiration: unreviewed Tasks stay visible until explicit disposition.
Predecessor KRs and outcome evidence remain historical evidence.

Remaining implementation details: compose the existing chapter skills, specify
retained candidate output between steps, and settle the previously noted config
write ownership and chapter metadata representation. No skill implementation,
provider mutation, or Flow navigation decision was made in this review.
