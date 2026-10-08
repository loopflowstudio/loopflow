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
remain implementation choices requiring evidence. The resume repair now uses saved Session placement for context and provider cwd,
while retaining the LF Process caller cwd. Its CLI fixture proves this boundary.

2026-10-08: the real Codex client's single structured steer omits native skill
expansion; duplicate RPC ids repeat model input. The same source expands on a
fresh turn. Extending `send_current` cannot satisfy native skill invocation or
exactly-once application. The native-boundary queue/recovery approach still needs
LF integration. The later
Codex socket probe proves lost-reply correlation through `clientUserMessageId`,
connection handoff and native terminal draft preservation; it does not prove LF
admission or driver authority.
No replacement terminal, weaker fidelity, automatic ambiguous retry or new
provider protocol is selected. This is observed implementation evidence, not a
new product decision attributed to Jack Heart.

2026-10-08: the capture-key-as-native-message-ID candidate is superseded by LF's
retry path: one capture can contain an original invocation and a different
continuation. A per-dispatch native ID is now retained beside its capture and exact
input before the fenced send. This is an implementation choice, not Jack Heart's
new product decision. Future pending-input consumption must reuse that retained
ID through handoff; it must not generate another ID and resend uncertain input.
Native receipt observations do not settle delivery or establish native expansion.

2026-10-08: a two-client Codex race invalidates idle-read plus turn/start as
boundary admission. The `--queue-race` proof demonstrates existing native
`thread/queue/add` admission, preserving a competing turn, sibling work and native
terminal draft through dropped enqueue acknowledgement and connection handoff.
`ed9ad8a61` also proves terminal-free consumption and recovery of two still-pending
inputs after engine death. Native resume can empty the queue before history shows
the input; absence during that gap cannot authorize resubmission. This mechanism
remains the LF integration candidate, without a provider extension or replacement
terminal. Admission, capture attribution, matching completion and teardown remain
one coordinated repair. These are observations, not new decisions by Jack Heart.
