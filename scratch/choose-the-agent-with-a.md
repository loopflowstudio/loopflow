# Global agent selection (LOO-440)

Jack Heart requested one global `-a` / `--agent` declaration on October 8,
2026. His later steer gives an explicit selection precedence over Task, Flow,
step and skill agents, including nested child commands. Without the flag,
existing definition/default selection remains. Delivery stops at demo; no
logins, credentials or real provider accounts may be used.

## Baseline

Main baseline `86d0e5e6a` builds. All six supplied command forms reach semantic
dispatch in a fresh fixture Machine; none misreads the agent as a skill. Unknown
Task/repository/Flow diagnostics are expected in that empty fixture and prove
parsing only. Root help silently ignores selection. Source shows saved Task
agent precedence and no inherited override for agent-issued child commands.

## Reconciled implementation (2026-10-08)

One Clap global replaces PR-local declarations and precedence. Generic argument
partitioning still serves external skills and preserves literal `--` arguments;
duplicate parser tests and the separate before/after lists are removed.
Help and the generated reference describe the global override once.

`LF_AGENT_OVERRIDE` carries explicit selection before prompt preparation and
capture. Both prepared and direct provider launches apply it; Task/Flow defaults
never become inherited overrides. Independent Session shells clear it. Remote
transport forwards inherited selection; a top-level remote flag stays in argv.
Replay changes only its new capture. Native resume retains its provider and
permits a model override, rejecting a provider change before launch.

Compression keeps one partition of global flags and skill arguments. The public
CLI fixture asserts the exact six Claude Sessions across Task, nested Flow and
provider-child launches, including a child that explicitly requests Codex.
Separate no-flag launches retain skill/step models. Native-history refusal keeps
all six Sessions. Synthetic accounts use `--isolate`; an earlier shared activation
failed at a macOS Keychain write without using real providers or logins.

Main's startup changes from `497885050` are integrated. They remove provider
probes and reuse resolved directories; the override does not restore those
probes or require another resolver. Release's child memory was read in full:
its operation-entry lesson supports checking recorded Sessions through the
public CLI, rather than treating successful parsing as launch acceptance.

## Remaining work

Publication checks covered Task launches, replay, native resume, shell isolation,
command discovery and startup after the integrated main changes. Review found
root help exceeded its existing 25-line limit; removing two blank lines restored
the limit without dropping the global override explanation. Discovery then passed.
Demo/review owns judgment using synthetic providers under Jack Heart's constraint.
Remote execution, installed/live-provider acceptance, hosted CI and delivery
remain unproved; real-provider use and landing remain outside authorization.
No product decision is needed for this reconciliation.
Prior evidence: `69bdad343:scratch/choose-the-agent-with-a.md`; compression:
`5df2ba251`. The publication repair changes only root-help spacing.

Checks: network-isolated `cargo nextest run -p loopflow` across Task launch, Session CLI, startup, discovery, parser, replay, resume, shell and Flow-graph cases: 77/78 passed; after the help-spacing repair, `--test cli_discovery`: 23/23 passed; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `git diff --check` passed. Full gate and hosted CI remain unproved.
