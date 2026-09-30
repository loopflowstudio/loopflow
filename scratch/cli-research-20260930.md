# CLI research applied to LOO-338

Jack Heart requested a fresh comparison on 2026-09-30. This pass reads all six
historical artifacts at `953ca9814:scratch/`: `research-cli-comparisons.md`,
`cli-command-map.md`, `cli-command-graph.json`, `cli-shorthand-comparison.md`,
`cli.md`, and `questions.md`. Their original counts and proposed names are
historical; the [compiled catalog](cli-command-catalog.md) is this branch's
baseline. Recommendations below are implementation requirements unless marked
as a real demo choice. No external service was mutated.

## Fresh comparison

Official references checked on 2026-09-30. These are documented contracts,
not a claim that each installed binary was exercised.

| Dimension | Current primary evidence | Consequence for lf |
|---|---|---|
| Verbs and ownership | gh separates PR list/view/status and mutations. Docker documents `container ls` and several equivalent spellings. [gh PR](https://cli.github.com/manual/gh_pr), [Docker](https://docs.docker.com/reference/cli/docker/container/ls/) | Use object owners, but do not copy Docker's alias multiplication. List enumerates, show inspects, status summarizes changing state. |
| Help and discovery | gh accepts nested help paths. Cargo documents both nested help and leaf `--help`. [gh help](https://cli.github.com/manual/gh_help), [Cargo help](https://doc.rust-lang.org/cargo/commands/cargo-help.html) | One read-only explanation per operation; preserve reserved-name escapes and command precedence. Delete duplicate `skill show` rendering. |
| Machine output | Git porcelain v1 explicitly promises stable parsing independent of configuration. gh offers JSON fields and optional query/template processing per supported command. [Git status](https://git-scm.com/docs/git-status), [gh formatting](https://cli.github.com/manual/gh_help_formatting) | Stable DTO identity, explicit missingness, no ANSI/progress on JSON stdout. Do not invent a global formatter or promise every command returns the same shape. |
| Account/context selection | AWS profiles bundle settings and credentials. Stripe's project-name selects CLI configurations across accounts. Vercel distinguishes project and scope selection. [AWS](https://docs.aws.amazon.com/cli/latest/userguide/cli-configure-files.html), [Stripe](https://docs.stripe.com/cli), [Vercel](https://vercel.com/docs/cli/global-options) | Keep account, Work, model, Home and browser venue distinct. Jack selected account; profile would collide with the existing browser concept. |
| Preview and confirmation | kubectl distinguishes client and server dry runs. gh repository deletion requires an explicit target for unattended `--yes`; ambient deletion still prompts. [kubectl conventions](https://kubernetes.io/docs/reference/kubectl/conventions/), [gh delete](https://cli.github.com/manual/gh_repo_delete) | Name preview effects precisely. Confirmation, overriding dirty-state protection, and provider permissions are different controls. Do not merge them into one force switch. |
| Exit codes | gh documents success, failure, cancellation and authentication codes; AWS distinguishes parsing, configuration, service errors and SIGINT; Cargo uses 101 for failure. [gh](https://cli.github.com/manual/gh_help_exit-codes), [AWS](https://docs.aws.amazon.com/cli/latest/userguide/cli-usage-returncodes.html), [Cargo](https://doc.rust-lang.org/cargo/commands/cargo-help.html) | No universal numeric taxonomy exists. Preserve lf's 0/1/2/130 behavior, document it and test it. A richer taxonomy is a demo choice, not an arbitrary breaking change. |
| First local result | uv documents local project initialization and explicit project-free execution. [uv](https://docs.astral.sh/uv/reference/cli/) | Planning registration must not precede a direct local result. Start in a fresh directory with direct execution and required provider access; introduce no synthetic Task. |
| Error messages | gh deletion docs state both missing authority and the repair command; AWS distinguishes environment failures from service failures. Sources above. | Errors name operation, selected target, failed boundary and actionable next command, without secrets. A successful wrapper exit cannot imply the requested result occurred. |

Fresh jj and Fly page requests failed in the browsing tool. No new jj/Fly claim
or recommendation rests on those failures. The earlier Fly comparison remains
dated evidence only. This pass does not need every example CLI to exhibit every
dimension; it uses the primary sources that directly support each conclusion.

## Changes tied to catalog rows

| ID | Rows | Concrete change | Acceptance |
|---|---|---|---|
| R01 | C026–C059, C065–C078, C122, C129 and their argument rows | Apply object owners; account replaces auth, without identity/id. PR delivery works without Task registration. Move Home/Wave controls and update all consumers. | Canonical parser paths, derived shorthand, Desktop argv and typed Flow dispatch agree. No predecessor aliases. |
| R02 | C120–C121, C139–C141; all extra aliases | Merge `skill show` into typed help. Delete extra uppercase flag aliases and worktree `rm`; retain the selected monitor/mon exception. Help names exact canonical paths and effects. | Equivalent help forms, reserved definition names, unknown commands and ambiguous shorthand execute no effects. |
| R03 | C039 `--format`, all `--json`, C124 `--watch` | Replace worktree's permissive `--format` with `--json`. Preserve existing DTO fields through ownership moves; migrate Exec/Session fixtures with LOO-298. Explicitly distinguish one JSON document from NDJSON streams. | Parse complete stdout as the promised shape; progress/errors on stderr; no inferred zero for missing evidence. |
| R04 | Root `--account`, `--only-account`, `--model`, `--as`; C044–C051, C097–C099, C129 | Account overview and launch-owned readiness use the same account facts. Destination children recheck required access while retaining inherited restrictions; preference remains distinct from restriction. | Foreground/background/remote behavior tests and a real local provider result; no credential-copy shortcut. |
| R05 | C039 `--full`, C080 `--no-sync`, C128 | Delete proven ignored flags and retired `op` rejection namespace. Remove callers, parser fields and tests for those spellings. | Old flags reject; cached status and worktree listing retain behavior; no phantom compatibility branch. |
| R06 | C003, C099, C122, C130, C079 | Deliver first local result without planning account; Task list includes unlinked work and Wave list includes authored local goals. | Disposable public CLI journey using real provider output, plus separate fixture coverage of connected planning. |
| R07 | All plan/preview/dry-run and force/yes/yolo rows | Keep present distinct safety/authority behavior. Document effects and preview scope. Offer a single preview spelling and destructive-confirmation policy as explicit demo choices. | No hidden mutation in ordinary reads; headless paths never unexpectedly wait for stdin. No new universal confirmation gate before Jack's decision. |
| R08 | C031, C125, C118 | Keep draft publication in PR open for now; replay and process prune retain explicit effect descriptions. Demo choices: make open read-only; relocate effectful monitor children. | Preserve current draft/ready boundaries and exact signal authority until Jack selects a change. |
| R09 | C121 and all command failures | Publish current exit semantics (0 success, 1 operational/definition failure, 2 syntax/lookup failure, 130 SIGINT) and actionable canonical-path errors. | Public CLI tests inspect code/stdout/stderr for syntax, missing definition and failed operation. Authentication-specific codes remain a demo choice. |
| R10 | C043 `--push`, C124 `--resume`, C114–C125 | Merge commit-and-publish into PR open; provider continuation into Session open. Monitor overview preserves live/recorded/missing distinctions with reason and next action. | Retain commit-only endpoint, stable Session identity and truthful missing observations. No new Run command or relabeled Run DTO. |

“Keep” in the catalog does not mean keep stale Run vocabulary. LOO-298's naming
and identity changes must be integrated before monitor implementation. This is
an implementation dependency, not permission to defer the accepted outcome.

## Earlier recommendations: disposition

- **Organize by real work:** retained as R01/R06. The Task namespace never
  requires manufacturing a Task for an ordinary branch.
- **Unified help/discovery/collision contract:** already partly implemented;
  R02 finishes redundancy removal and proves the moved tree. The earlier
  cross-kind ambiguity-error suggestion was superseded by Jack's accepted
  flow-first rule in the historical design. Do not restore it.
- **Selection versus authority/evidence:** retained as R04. The earlier
  plausible auth/identity owner is superseded by Jack's account decision.
- **Read versus mutation semantics before renaming:** retained as R03/R07/R08.
  No gh-style reinterpretation of PR open without the demo decision.
- **Fixed aliases and account naming follow-ups:** account is now selected.
  Retain monitor/mon only; further aliases contradict the newer culling rule.
  Primary lowercase short flags and their long form remain one option.
- **Local-first Wave/Task discovery, first entry and account inheritance:**
  retained as R04/R06, including authored Wave discovery and unlinked work.
  These were omitted implementations, not expendable documentation proposals.
- **Usage ranking:** still an unselected experiment. No evidence here justifies
  unstable ordering; stable catalog/JSON ordering remains required.
- **Concrete type removal:** the recovered design explicitly proposes a
  separate runtime/persistence consolidation. It does not implement a CLI
  convention or first-result requirement and would overlap LOO-298's captured
  Session model. Do not independently rewrite that authority in this CLI Task.

## Demo choices for Jack

1. Keep draft-publishing PR open, or separate read-only browser viewing from
   draft preparation. Show the precise effects and existing Desktop caller.
2. Keep replay/prune under monitor, or place mutations under their execution
   and Home owners. No extra alias or command is needed merely for aesthetics.
3. Standardize preview spelling across domains; distinguish local computation
   from remote validation. Current preview controls remain functional meanwhile.
4. Add explicit unattended confirmation to destructive operations, or retain
   explicit-target/authority semantics. Do not conflate `--force` with consent.
5. Add authentication-specific exit codes, or keep the documented existing
   contract. Show affected scripts before any numeric change.

Account has no proposed abbreviation in this pass. Before/after counts must
come from the compiled final tree, not estimates from the rename table.

## New counterexample during review

A disposable public-CLI check returned the reserved skill body for
`lf skill show list`, but `lf help skill -- list` returned command help for
`skill list`. Therefore R02 must preserve the delimiter through Clap and inspect
that definition before deleting `skill show`. The separate expression
`lf skill -- list --help` is execution with a literal argument after the delimiter;
it timed out after five seconds and is not an inspection proof or a first local
result. The test process was terminated by Python; no provider result was observed.
Use explicit `help` for this regression, with provider executables absent.

The ownership review also rejects the draft's `home tokens` placement:
`lf/commands/tokens.rs:46` requires a repository and measures tracked source or
Git history. Its catalog verdict is **repo tokens**, preserving its distinct
purpose from provider consumption. This is an adopted object-ownership repair,
not a new operation or an unresolved preference.
