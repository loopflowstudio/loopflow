# Questions

- Dropped `-y` from the documented install (README, docs/index.md,
  docs/agent-api.md, skill README) so the person sees the security assessments
  and confirms. That adds the installer's agent and symlink/copy prompts. Restore
  `-y` if a one-shot install matters more; the PromptScript failure line then
  comes back (installer bug, harmless).
- The PromptScript `-g -y` behavior is a skills CLI bug worth reporting upstream
  (vercel-labs/skills); not filed, since that's an external post.
