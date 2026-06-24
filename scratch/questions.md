# Open questions / assumptions

Current to the steps-as-skills milestone. Resolved decisions live in `steps-as-skills.md` and `release/unreleased/DECISIONS.md`; keep this file for assumptions that still need reviewer attention.

- **Global vendor discovery.** Repo-local skills are structurally verified and have been live-probed for Claude/Codex. Global sync writes the same shape under `~/.claude/skills` and `~/.agents/skills`, but live global discovery still depends on each vendor's runtime behavior outside this repo.
- **Namespaced skill invocation.** Builtin namespaced steps sync into nested skill directories. Assume vendors accept `/gstack/office-hours` for Claude and `$gstack/office-hours` for Codex; re-check before relying on namespaced app handoffs as the primary path.
- **External skill fallback.** `npx/*` and `rams/*` steps still use the assembled prompt path, because their source of truth is already vendor-specific skill cache content and loopflow should not mirror it as a generated skill until namespace semantics are designed.
- **Directions removal is deferred.** `direction` is still a first-class wave/config/DTO field. Removing it is a separate migration under the DTO fixture discipline, not part of this PR.
