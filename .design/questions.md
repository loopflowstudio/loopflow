# Open Questions

## lfops land behavior when auto-merge is disabled

**Context:** `lfops land` tries to enable auto-merge via `gh pr merge --auto --squash`. When the repo has auto-merge disabled, it fails with a message telling the user to either enable auto-merge or run `gh pr merge --squash` manually.

**Question:** Should `lfops land` automatically fall back to waiting for CI and then merging (e.g., polling PR status), or is the current "inform and exit" behavior correct?

**Considerations:**
- Current behavior is explicit and lets user decide
- Auto-fallback could be surprising if user expected auto-merge
- Polling for CI completion adds complexity and potential for hanging
- Manual `gh pr merge --squash` after CI is simple enough

**Recommendation:** Keep current behavior. The user message is clear and actionable.
