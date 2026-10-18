# Pi interactive demo — 2026-10-18

Jack Heart invoked demo review of [LOO-507's design](loo-507-add-pi-as.md).
The published candidate is `bf6c3d9b28`, PR #1591. Earlier isolated proofs are in
[the maintained probe guide](../scripts/benchmarks/pi/README.md); configured
OpenCode/Gemini access is recorded in [authentication evidence](pi-authentication.md).

Opened cmux workspace `workspace:1000070043`, named **Pi demo**, with the candidate
binary and explicit `pi:opencode/gemini-4-argon`. Candidate reports
`0.14.6+f605333f0.dirty`; it was built from the verified precommit implementation.
The installed `lf` reports 0.14.5 and is not the demo binary. The candidate uses
disposable LF_HOME at
`/private/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loo507-demo-xvoj1qi4/machine`;
Pi retains its own configured login. No credentials were inspected or copied.

The launcher `run.sh` in that directory starts a native Session in the Task
checkout, requesting a read-only context summary and retention of the phrase
`cedar snow 507`. After Pi exits, Enter reopens the recorded Session; q finishes
the launcher. It does not automatically send another model request.

Observed: cmux confirmed workspace creation and the candidate process started.
Jack Heart asked where the demo was; it had not opened at that point, and this
was corrected before reporting the confirmed workspace. The native Pi response
then correctly identified the checkout, retained `cedar snow 507`, and summarized
the supplied Pi design without file-reading tools. The demo Session is
`session_642701f24f6a42708d51f233ff775c56` in its disposable Machine.

Jack Heart reported the large startup **Skill conflicts** warning list. It names
shared skills with slash-containing names (`repo/operate`, `task/session`, etc.)
and synced names longer than 64 characters. Installed Pi 1.3.0's
`dist/core/skills.js` validates names against lowercase letters, digits and hyphens,
with a 64-character limit. Its loader retains these skills despite name warnings;
the problem is real startup noise and incompatible exported metadata, not an
authentication or extension-load failure. Both cmux and Loopflow extensions loaded.

Source inspection points to Loopflow's skill export in
`rust/loopflow/src/skills/mod.rs`: exported metadata uses the logical skill name.
Proposed correction: export valid, short, collision-safe native names while
preserving canonical Loopflow identity and original source/assets. Verify both
namespaced and long synced skills with real Pi discovery, without disabling
skills or suppressing diagnostics. This is implementation direction proposed from
the observed defect, not a user-approved naming design. No personal skill files
have been rewritten. The isolated probe homes did not exercise this shared catalog.

Jack Heart subsequently accepted shortening names rather than strictly preserving
the existing setup, provided the loss stays small, and authorized `lf ship` through
verified landing on main after the fix. This supersedes the proposal to relocate
Codex-only slash aliases. Selected implementation: one portable native export per
source for both shared and Claude directories. Keep ordinary flattened names;
normalize punctuation/case, shorten long names to the leaf plus a stable digest,
and suffix collisions instead of dropping skills. Preserve canonical `lf`
identity, native declarations, source/assets and third-party bundles. Native slash
shortcuts become dashed shortcuts; no separate alias directory or resolver is added.

Remaining visual coverage: inspect initial context summary, edit a Unicode draft while
resizing, exit and reopen, then ask for the remembered phrase. Confirm the
same conversation and readable terminal. Record Jack Heart's feedback here;
launch alone did not establish demo acceptance. Jack Heart later authorized shipping
after the naming correction; that is delivery authority, not retroactive test evidence.
Before successful demo sign-off, resolve the reported skill-catalog compatibility
and rerun this configured startup. The retained Session remains available for runtime review; do not interrupt it for
the export correction.

Source review found a pruning edge case: a blocked portable replacement must keep
its older slash export. The retained replacement test covers that behavior; the
new collision tests require both original skills/Flows to survive under distinct
portable names. Pi 1.3.0's real loader reports 85 skills and 59 diagnostics against
the pre-fix shared catalog. Verification will compare the same catalog after sync;
third-party files must remain byte-identical.

The naming repair passed 26 focused skill/catalog/invocation tests and the candidate
binary build. Candidate `self sync-skills --yes` used a disposable LF_HOME, updated
only the generated catalog (200 writes, 53 prunes, two protected destinations),
and preserved all 27 fingerprinted third-party SKILL.md files byte-for-byte.
Pi 1.3.0's own loader on the same real shared directory now discovers 101 skills
with zero diagnostics (previously 85 and 59). Both synced PDF sources survive as
`pdf-0ba5634c` and `pdf-746376d6`. Repeat sync writes/prunes nothing. Candidate help
resolves both `repo/operate`/`repo-operate` and `task/session`/`task-session` to the
same builtin. This is actual loader/CLI evidence, not a rendered `/reload` review.
The generated files were backed up locally before sync; no credential files or
running native Session were changed. Jack Heart was directed to `/reload` in Pi.

Final naming-repair checks: focused Rust suite (26), binary build, all-target Clippy, formatting and diff checks passed. The ship Flow owns the remaining affected gate, sync, publication and waited landing.
