# Implementation assumptions

2026-10-07, LOO-420: adopted the design's proposed repository-before-personal-before-
embedded ordering, with `.lf/skills`, `.claude/skills`, `.claude/commands`,
`.agents/skills`, `.codex/skills`, `.codex/prompts` within each scope. This is a
reversible implementation choice, not a new decision attributed to Jack Heart.
The full native-controls and same-conversation requirements remain unchanged.

2026-10-08, LOO-420: local Claude headless delivery uses private capture-local
plugins with retained SKILL.md bytes and links to existing sibling resources.
This reversible implementation choice preserves original provenance separately
from the native snapshot path; it does not claim unchanged parent-relative paths
or asset availability after bundle removal. Jack Heart has not accepted those losses.

Codex 0.160.1's captured-path transport needs a revised design: explicit input
requires native catalog membership, while its available extra-roots mutation
replaces engine-global roots. The local prototype was removed after reproducing
the conflict. Retain the full native/same-conversation scope; do not grant one
Session authority over sibling catalogs or replace native controls.

2026-10-08: a disposable Codex catalog probe passes with a unique native-name alias
and a repository symlink, preserving sibling roots and fresh plain skill selection.
An original-name mount breaks plain selection. This candidate is not in production;
placement, lifetime, native-name semantics and complete resource/control fidelity
remain implementation choices requiring evidence. The headless admission repair
preserves stored workspace attribution, while resume still assembles its context
from the caller's cwd; target-workspace assembly remains to reconcile.
