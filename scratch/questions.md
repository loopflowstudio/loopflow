Recovery assumption: use a repository-local write-docs agent override to Codex while Claude is cooling; validate launch with a bounded no-edit Task run, then stop it. Do not continue the design or rewrite in this incident run.

Deployment choice: validate the installed patch in a disposable Task-free no-op launch with ordinary manabot write-docs routing, keeping the override until fallback and shutdown pass. This avoids another MTG-344 Graph launch. If real Claude refusal is no longer available, leave ordinary-fallback acceptance deferred and keep the override.
