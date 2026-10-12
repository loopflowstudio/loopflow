# How lf breaks for friends

Draft — 2026-10-11. Jack Heart supplied the direction below; implementation
mechanisms and supported boundaries remain proposals.

## What to build

Make lf fit an unfamiliar repository's existing practices without mandatory
configuration, while making essential requirements and unsupported uses clear.

Jack's framing:

> Imagine I give lf to my friends to use. How might it break?

> Probably ok to require using worktrees

> Where you put the worktrees should be pretty customizable

> lf's in "new" repos should observe whats going on and align with existing protocols, but move to lf's standards if tehre is none

This supersedes the earlier proposed restriction to a first-use smoke test.
The question spans adoption: distinguish essential mechanics, incidental
assumptions worth generalizing, and uses lf cannot support yet.

## Placement and coordination

Wave unresolved; no Wave supplied. Companion: `levels-of-lf`.
Its 2026-10-11 draft now describes a small launcher instruction contract and
additional guidance when Work APIs are used. Repository instructions remain
authoritative; taskless Sessions, Skills, Flows, worktrees and PRs remain useful.
That branch owns instruction allocation. This branch investigates whether the
underlying operations honor the conventions those instructions defer to.

## Shape and proposed keystone

Additive series. First map concrete ways friends' environments differ, then
select coherent repairs. Worktree placement is the leading candidate: retain
worktrees as a likely essential mechanic while removing sibling-directory
placement as an assumed requirement. Jack's “probably” remains provisional.

Inspect these seams, separating observed code from hypothetical failures:

| Variation | Design question |
| --- | --- |
| Existing worktree layout | Can lf reuse and create work where this repo already puts it? |
| Review host | Can publication, status and completion use the actual host? |
| Review procedure | Can teams retain manual review, custom checks and merge policies? |
| Planning | Can launcher-only use and existing Git/Linear choices stay independent? |
| Provider, OS, permissions | Which are supported, discoverable, or explicit limits? |
| Build/test conventions | Do selected skills follow repo commands rather than impose this repo's tools? |

No implementation launches or follow-up Tasks are implied by this inventory.

## Current system

`git/worktrees.rs::dir_for_component` always creates sibling paths shaped
`<repo>.<name>`. `worktree_dir` feeds named creation and `plan_placement`.
`sibling_worktree_name_with_main` deliberately excludes non-sibling layouts;
selectors and CLI display use it. Git's registered worktrees already provide
actual checkout paths. Placement changes must include consumers, not merely
change the constructor.

`ops/pr.rs::create_or_update_pr` commits/pushes, creates or updates through `gh`,
and attaches GitHub identity to the Task. `work/task/mod.rs::PrPublication`
contains `Option<GithubPr>`; Task PR observations and merge validation also name
GitHub. `ops/pr_landing.rs::observe_github` reconciles delivery. Replacing only
`lf pr open` with an arbitrary command cannot make that lifecycle host-neutral.

Flows and Workflows already express procedure. `docs/lf.md` documents Git-backed
planning as the default and optional Linear; `docs/getting-started.md` still says
Tasks and Waves require Linear. This is a concrete documentation inconsistency.

Existing installer, installation-isolation and provider fixtures remain useful;
installer smoke alone does not prove a complete useful experience.

## Proposed resolution rule

Honor explicit choices and repository instructions; reuse actual existing work;
infer an unambiguous convention from relevant repository evidence; otherwise use
lf's default. No setup questionnaire or generated config required for ordinary
entry. Existing odd or conflicting layouts are not permission to relocate work.

Keep semantic intent stable while varying mechanisms. Separate review host
(GitHub, Bitbucket, etc.) from review procedure (people, checks, sequencing).
Use authored Flows/Workflows for procedure before adding parallel policy knobs.
A configurable command could implement an operation, but success must have a
meaning the next operation can consume: identity, published revision, observed
state. Exit zero alone cannot mean merged or Task complete. Whether to support
external implementations at all remains open; no plugin framework is selected.

## The demo

Enter a repo with an established non-sibling worktree layout and no lf config.
Create work through lf; its location follows that convention, existing work stays
put, and listing/selecting/resuming agree on the checkout. Repeat in a repo with
no convention: lf uses its normal default. A team with custom review keeps that
procedure instead of silently receiving lf's preferred merge behavior.

## Data structures and key functions

Retain `WorktreeName`, `PlacementPlan`, Git's worktree registrations and existing
Task/Session identity. The private `inferred_worktree_dir(repo: &Path, component: &str) -> Option<PathBuf>`
feeds the existing directory resolver used by named creation and `plan_placement`.
No new schema or saved repository-profile object is currently justified.
Review-host adaptation requires a separate full lifecycle design if selected.

## Delete — do not maintain

Replace unconditional sibling construction in `dir_for_component`. Inspection
found selector/display consumers already fall back to branch and directory names;
retain their useful sibling parser and prove non-sibling behavior through the CLI.
Preserve collision protection, branch naming and existing checkout reuse.

## Forbidden outcomes

Mandatory lf config before using existing conventions; agents and native commands
choosing different locations; inferred layouts relocating existing work; successful
custom publication followed by GitHub-only status; script exit treated as merge
proof; a broad provider framework without a concrete second use case.

## Internal slices and done when

**This slice:** implement and demonstrate worktree layout discovery. Jack requested
continued work and a demo on 2026-10-11. The chosen implementation uses at least
two live, branch-bearing Git registrations with the same parent directory and
the same filename prefix before the branch leaf. Ignore primary, detached and
missing checkouts. Conflicting examples or insufficient evidence use the default;
existing branch checkout reuse wins. This is a bounded mechanical discovery rule,
not natural-language interpretation of repository instructions. No override
configuration is introduced in this increment.
Follow-ups: review-host lifecycle, team procedure compatibility, further supported
boundary repairs only where the inventory identifies concrete value.

Headless acceptance for placement must cover established layout, existing branch
reuse, no convention, conflicting evidence, and consistent selection/resume.
Gate: `cargo test -p loopflow --test worktree_tests` under the repository network
isolation wrapper, followed by `cargo clippy --all-targets -- -D warnings` and
`cargo fmt --check`. Expected: layout discovery, defaults, reuse and CLI selection
pass alongside the existing worktree suite. Real demo/review judges whether discovery matches a friend's expectation.

Check: isolated `cargo test -p loopflow --test worktree_tests` — 27 passed; `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed; 21 worktree library checks passed; `cargo build --bin lf` passed.


## Demo preparation — 2026-10-11

Implementation checkpoints: `8478f3826b`, `22f2bc1ddf`. Automated fixtures exercised real CLI
creation and selection in nested and external layouts without lf configuration.
No participant has yet reviewed or accepted the demonstrated behavior.

To inspect placement without creating anything, run the checkout binary inside
an example repo: `/Users/jack/src/loopflow.how-will-lf-break/target/debug/lf wt
create fix-login --plan`. With `.worktrees/search` and `.worktrees/payments`
on corresponding branch leaves, expect `.worktrees/fix-login`. Run without
`--plan` to create it, then `lf wt switch fix-login` to select it. Use the same
checkout binary for each command until this change is installed.

The baseline smoke restriction is superseded. Remaining broader work is the
assumption inventory and review-host/procedure boundary; this increment does
not claim general compatibility with every team's conventions.

Review finding: selector/display fallbacks already handle alternate layouts; retain
them rather than replacing working behavior. Corrected sibling-only CLI help and
reference text. No general configuration or provider abstraction was added.
