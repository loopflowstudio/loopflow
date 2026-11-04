# Configuration organization — November 4

Jack Heart requested Ghostty and VS Code as references. Jack wants agent and
context configuration (agent, docs, etc.) primary, with separate Desktop,
Release and Planning sections. Jack subsequently requested top-level Machines
and Accounts sections and questioned the Metadata category. Jack accepted the
organization below, requested personal/repository config updates, then asked to
try the viewer first. Configuration-file migration is deferred until that review.

## Prior art

- [Ghostty reference](https://ghostty.org/docs/config/reference): flat keys,
  related name families, documentation ordered roughly by common use and related
  options. It does not provide nested configuration sections.
- [Ghostty configuration](https://ghostty.org/docs/config): CLI flags share config
  keys; website, installed references and `+show-config --default --docs` are
  generated from the same source. Preserve that agreement across LF surfaces.
- [VS Code configuration schema](https://code.visualstudio.com/api/references/contribution-points#contributes.configuration):
  categories, ordering, names and descriptions drive settings UI and JSON editing
  assistance. Category is independent of declaration source and scope.
- [VS Code settings](https://code.visualstudio.com/docs/configure/settings):
  feature groups and Commonly Used navigation; file and UI edit the same settings.

## Proposed direction

Keep concise agent keys and a context block prominent at the root, including skill/Graph
exceptions. Give auxiliary systems explicit namespaces and matching viewer/docs
sections. Do not force everything beneath a generic settings or agent wrapper.

Primary: agent choice/fallback, context.docs/exclude/budgets, execution
options, and targeted skill/Graph declarations. Machines and Accounts each have
their own top-level section, as Jack requested.

Other sections: Desktop; Release; Planning. Drop the vague Metadata category.
Accepted grouping: a small Loopflow section for user.name, installation
updates and diagnostic logging. Exact renamed paths remain to be designed.
Separate project release targets from the installed LF update stream. Inspect
actual consumers before classifying shared appearance/session behavior as
Desktop-only; detached Session lifetime also governs CLI-created Sessions.

Extend the existing executable catalogue with category and intentional ordering;
use its descriptions, schemas and applicability in CLI, Desktop and generated
reference material. Nested settings need useful descriptions at their own paths.
Keep configuration paths visible alongside friendly labels. Categories must not
change precedence, allowed scopes, targeting, portability or write destinations.

Remaining design: map every existing setting to its owner; choose actual renamed
paths and migration boundaries, especially theme, stream, session and user.name.
The viewer alone cannot fix ambiguous configuration-file names. Preserve values
and existing repository edits when applying Jack's requested migration after
the viewer review; the currently installed parser must still read shared files.

## Viewer preview

The catalogue now assigns every setting one feature category. CLI inspection and
catalogue browsing use it; Desktop's Browse menu reads the same category and
ordering. The compact table hides empty groups and keeps actual setting paths.
Agents and context is first; Machines, Accounts, Desktop, Planning, Release and
Loopflow follow. Session lifetime remains under Loopflow because it also applies
to CLI Sessions. The primary agent row retains default_agent in its explanation.

Try `./scratch/config-demo.sh config show --skill design`, optionally with --all
or --explain. The temporary fixture contains examples in every group. Personal
and repository configs have not been changed. The first preview changed only
presentation; the accepted context namespace below now changes paths as well.
Resolution/precedence behavior is preserved. Evidence and current review state
are in settings-demo.md and config-grouping-evidence.json.

## Context namespace — accepted November 4

Jack Heart accepted context.docs, context.exclude and context.diff. Apply the
same organization to diff_files, paste and budgets, preserving each leaf's merge
and portability policy. The redundant former context path list folds into docs.
CLI --docs/--diff/-c retain their spellings and produce these canonical paths.
Skill and Graph preferences use the same namespace; durable Skill.docs remains
the existing capture representation, projected into context.docs for resolution.

Keep released files readable during rollout using one temporary normalization
boundary, with a named removal Task as AGENTS.md requires. The catalogue must
describe leaf policies: the context mapping merges, docs/exclude append or clear,
booleans replace, and clipboard selection remains host-local during forwarding.
Update docs, code and the demo together; prepare compatible personal/repository
migrations without installing a branch binary or breaking the installed parser.

The canonical namespace is implemented locally and demonstrated by the branch
CLI. Validation and review boundaries are recorded in settings-demo.md; commands
and outputs are in config-context-evidence.json. Personal settings needed no
context migration. The two repository patches remain staged until a compatible
release is installed. No additional namespace renames or installation are part
of this preview.
