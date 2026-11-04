# Settings viewer demo — LOO-714

## Context namespace preview — November 4

Jack Heart accepted grouping the supplied inputs under context. The branch now
uses context.docs, context.exclude, context.diff, context.diff_files,
context.paste and context.budgets consistently in files, skill/Graph settings,
CLI setters, inspection and Desktop catalogue entries. CLI flag spellings stay
the same. The temporary fixture uses the canonical mapping; the real CLI shows
nested paths, and --all includes their defaults. --explain preserves provenance.
Captured commands/output: config-context-evidence.json. Jack has not yet reviewed
this namespace preview. The earlier grouped preview below predates the rename.

The focused namespace integration test passed: inspection agrees with prompt
preparation, exclusions remove supplied docs, CLI overrides target canonical
paths, [] clears inherited docs, and unset reveals them. Library checks passed
110 tests before the final nested-default-display repair; the CLI table test
covers that repair. All 35 affected integration cases now pass, including the corrected exported
Graph flag and two preview fixtures that needed an explicit harness after the
previous default-selection change. Formatting, all-target Clippy, the Desktop
build and both configuration Swift tests passed. Desktop linking emitted macOS
deployment-target/unwind warnings; no mounted Desktop interaction is claimed.
Logs: /tmp/loo714-context-{tests,checks,final-checks,completion-checks}.log. No provider was launched by the demo.

Nine main repositories were inspected for migration. The personal file needs no
context change. Loopflow and Hootro patches are in config-migrations/ and pass
Git apply checks; Loopflow includes gate/implement frontmatter. They remain
unapplied because the installed LF rejects the new mapping in a disposable
Home/project. No installed binary was replaced. Removal Task 807ef7 owns the
temporary released-file reader after rollout.

Run `./scratch/config-demo.sh config show --skill design`, with --all or --explain
as desired. Current design: config-organization.md. Task LOO-714 is idle, PR #1805
open and shipping unarmed; existing Graphs are completed/stopped. Disposition:
available for local review in this conversation, with no new Graph launched.

## Grouped viewer preview — November 4

Jack Heart accepted separate Machines, Accounts, Desktop, Planning, Release and
Loopflow sections, with Agents and context first, then asked to demo the viewer
before editing personal/repository files. The runnable CLI uses catalogue-owned
categories, keeps actual setting paths and hides empty groups. Existing --all,
--explain and JSON behavior remain. Desktop's Browse menu consumes the same
catalogue category; a CLI demonstration does not prove mounted Desktop behavior.

The temporary config-demo.sh fixture now includes examples for all seven groups.
The real CLI printed each in the intended order and retained source explanations.
The focused CLI regression passed, covering order, membership, empty-group hiding
and the existing agent/default explanation. The Rust catalogue fixture, Desktop
build, two focused Swift tests, formatting and all-target Clippy passed. Logs:
/tmp/loo714-config-groups-{test,catalogue,desktop,clippy}.log. A real PTY showed
colored keys/values and bold groups; NO_COLOR/plain output remained uncolored.
Jack has not yet reviewed this grouped output.
No real personal/repository preferences were edited and no provider was launched.

Run `./scratch/config-demo.sh config show --skill design`; add --all or --explain.
The runbook at scratch/demo.html shows captured before/after output and commands.
Design: config-organization.md. Captures: config-grouping-evidence.json.
Next: Jack reviews grouping and density, then map the accepted groups to concrete
file paths and perform the requested personal/repository migration compatibly.
Disposition: available in this conversation through the runnable local demo.
Fresh Task status is idle, pursue at end, PR #1805 open and settlement unarmed.
No Graph was started; the preview changes remain local for review.

## Explicit harness preference — November 4

Jack Heart accepted removing the implicit Codex-first ranking. The rebuilt CLI
shows the actual checkout's implementation agent as unresolved (null, no
candidates): multiple harnesses are installed and the personal Codex declaration
was deleted. The isolated demo still has an explicit Codex fallback; its wrapper
now announces its temporary settings on stderr. No personal preference was saved.
The CLI regression for zero/one/multiple installations and Task inspection passed.
Session continuation without a default, configuration/Graph suites, Rust/Swift
DTO checks, formatting and all-target Clippy passed. This
supersedes the earlier actual-checkout Codex observation below.

## Formatting feedback — November 4

Follow-up: Jack Heart requested one effective agent row, with default_agent only
inside that agent's explanation, including --all. Graph steps without an explicit
ID should display their skill name (pursue / implement), not “unnamed step”.
The CLI renderer and passing regression fixture cover both refinements; JSON and
selection semantics remain unchanged. The real demo's pursue graph shows step
names and one agent explanation. Formatting and all-target Clippy passed;
config-demo.sh and the captured HTML/JSON evidence use the rebuilt binary.

Jack Heart ran the design example and requested color and a better tabular layout.
The raw candidate dump was too noisy to scan. The local revision makes authored
settings a compact Setting / Value / From table, retains the effective agent, and
moves full provenance behind --explain and built-in-only rows behind --all. JSON
remains complete. Use existing terminal colors and NO_COLOR handling; preserve
resolver-owned outcomes. The CLI regression, a real PTY color check, NO_COLOR/plain output, formatting and
Clippy passed. Jack Heart accepted the revised CLI viewer: “config viewer looks good.”
Both valid --all flag orders produced identical output; Jack withdrew the parser
concern as a false alarm. No parser change was made. This changes presentation
only, not configuration precedence. Published as 9258bc4128 on PR #1805;
Auto-Ship remains stopped and automatic failure recovery remains PR2.

Branch naming history: 7b87719b8c added branch_names.schema in January 2026;
098bbcafa4 / PR #818 explicitly retired it in July when worktree identity replaced
custom schemas. After checking that history, the authorized removal deleted only branch_names.
Backup: ~/.lf/config.before-branch-names-removal-20261104-120834.yaml.
The real checkout now inspects successfully and selects the built-in Codex fallback.

## Configuration viewer — November 4

Jack Heart requested a demo of the configuration viewer. Jack ran the CLI design example and supplied its terminal output; this selected
the CLI demo. Jack Heart subsequently accepted its revised presentation above.
The branch CLI was exercised without provider launches or changes to installed
preferences. Four disposable-project reads selected Claude for shared design,
Codex for general implementation, Claude for a private Graph occurrence, and
Codex for its explicit CLI override. Both text and JSON outputs were captured.

The actual checkout read FAILED before inspection: personal config contains
`branch_names`, rejected as unknown/ineligible. No current consumer was found in
branch or main source. This was a concrete real-config cleanup gap. Jack authorized removal; after the
history check above it was removed with a backup. Real configuration inspection
now succeeds. No resolver exception or migration policy was introduced. No Graph was launched by this demo.

Try `./scratch/config-demo.sh config show --skill design`, add `--explain` or
`--all`, then try `--skill implement`,
then `--graph demo --step revision`; prefix the latter with `-a codex` to compare.
The wrapper selects an isolated Home/project and the branch binary. Evidence:
`scratch/config-viewer-demo-evidence.json`; runbook: `scratch/demo.html`.
Current contracts: `docs/config.md`, `docs/architecture/settings.md`.
The runbook contains captured real CLI output, not a Desktop mockup. Desktop UI
has not been demonstrated or photographed. CLI presentation acceptance is recorded above.

## Earlier demo — November 3

Jack Heart requested demo/status; design approval is not acceptance of the behavior.
The original demo observed PR #1805 at `4b424ecafa`, eight CI failures and a misleading
config-show agent warning. Complete dated record: `04bdec77e3:scratch/settings-demo.md`;
GitHub run 41629160735. settings-scope.md records subsequent approved repairs and
local passes. At `04bdec77e3`, the PR was open, auto-merge off, hosted Rust/aggregate
checks skipped. This is historical status; the current plan owns acceptance and
review-only delivery. Recovery is PR2.

The demo used a disposable repository/Home, inherited LF variables removed, and
a SHA256-identified branch binary. Eight successful commands proved personal skill
selection, distinct release occurrences, chrome=false over repo true, repeated
composition, DB set/unset restoration and CLI over environment in inspection.
The config-show warning contradicted its effective Codex result; the approved
revision repairs it with a launch/inspection regression.

No real provider ran. This establishes neither installation nor final-head proof.
Session/Task isolation and forced Graph behavior rely on recorded stand-in tests;
non-agent demo coverage was chrome. No new demo acceptance is recorded.

Historical review page: demo.html; raw commands/results: settings-demo-evidence.json.
settings-catalog.md owns the accepted extension and remaining work.
