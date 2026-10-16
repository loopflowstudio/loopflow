# LOO-494 assumptions

- The supplied untracked startup-fix-auto Flow/skill are local invocation inputs,
  not product changes. Preserve them in place and exclude only those exact paths
  from delivery; keep the supplied private evidence outside published commits.
- Interrupted-client recovery is the controlled causal reproduction. The original
  38-second incident has no stack sample; do not assert retrospective certainty.
- Use the release-run skill's default patch version if a new release is needed.
  Task completion waits for independent released startup acceptance.
