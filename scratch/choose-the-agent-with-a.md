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

## Implementation

Use Clap's global declaration and the existing generic handling of external
skill arguments. Carry explicit selection through one invocation environment
value; defaults must never create it. Resolve it before preparing a provider
prompt/capture, and preserve it through provider children and remote transport.
Fresh independent Session shells clear it with other invocation context.
Keep `--` opaque. Explain no-agent commands without changing their result.

## Delete — do not maintain

- `PrCommand::Publish.agent` and `PrCommand::Open.agent`, their precedence
  branches and duplicate parser tests.
- PR-specific expectation that reordering moves `-a` to a leaf. Clap owns
  global argument placement; generic external-skill handling remains necessary.

## Review and remaining work

One global declaration replaces the PR copies. Existing generic rewriting still
handles opaque external-skill arguments; inherited Clap globals are not local
argument owners. Explicit selection travels as `LF_AGENT_OVERRIDE`, while Flow
children stop turning saved defaults into `--agent` flags.

Review found two additional consumers: replay now records the selected agent on
its new capture without changing the source; native resume rejects a different
provider before launching and allows a model override within the same provider.
No schema change or compatibility path is needed. The generated command reference
lists inherited globals once.

The CLI fixture demonstrates the requested behavior using disposable Task/Workflow
records, nested Flows and stand-in providers. All six step/child Sessions record
Claude; separate no-flag runs record skill/step model choices. Native-history refusal
preserves all six Sessions. Real provider use, installation, hosted CI and delivery
were not exercised. Gate owns affected suites; demo/review owns judgment. No landing.

Realign inspected Release's headings and relevant publication/recovery sections;
its operation-entry lesson supports the public CLI fixture. Other child historical
incident detail was not reread. Infrastructure memory retains source versus installed
acceptance and archives older installation detail at the supplied base commit.

Checks: build, `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, Ruff and 32 focused network-isolated parser/launch/replay/history tests passed; affected suites deferred to gate.
