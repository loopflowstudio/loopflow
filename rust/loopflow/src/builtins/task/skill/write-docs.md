---
description: Write the intended user documentation before implementation, ready to read and edit while work continues.
requires: a chosen direction
produces: scratch/user-docs.md with draft user documentation and its permanent destinations
action_style: procedural
---
Write the docs someone will use when the change ships.

1. **Recover the chosen experience.** Read the request, settled decisions,
   prototypes and existing docs. Work from the chosen direction; do not reopen
   exploration or turn this into implementation planning. No preceding skill,
   Graph or Task is required. For a change with no user-facing documentation,
   say so briefly and finish without manufacturing a draft.

2. **Write the user docs.** Create or update `scratch/user-docs.md`. Lead with
   a realistic usage example and its expected result. Explain the concepts,
   prerequisites, options, defaults, failures and recovery needed to use the
   change; include only what matters to this experience. For a library, show
   a call site; for a CLI, commands and output; for an app, actions and what
   appears. Write publishable documentation, not a press release, architecture
   plan or list of promises to document later. Keep the scope proportional to
   the change: one revised paragraph can be enough.

   Mark the document as intended behavior, not a claim that it already works.
   Name the permanent documentation path and section for each draft excerpt,
   using the project's existing docs structure. Keep this scratch draft as the
   working copy until implementation integrates it; do not maintain competing
   copies. If a draft already exists, preserve saved edits and settled choices.
   Reuse sufficient documentation rather than rewriting it on every run.

3. **Read it as a user.** Can someone follow the example without knowing the
   implementation? Awkward explanations and surprising prerequisites are design
   feedback. Resolve details implied by the chosen direction. If writing exposes
   a consequential unsettled product choice, name the exact question in the
   working design and report it; do not invent acceptance. Keep any independent
   documentation useful and distinguish proposals from settled behavior.

4. **Make it available.** Put a short invitation above the draft:
   “This is what's being built. Edit and save these docs, or discuss changes in
   the conversation while work continues.” Loopflow Desktop surfaces Markdown
   under `scratch/` beside the conversation; no HTML wrapper is needed. Outside
   Desktop, give the same file path. Record the draft's path in the existing
   implementation plan so the next worker reads it from this checkout.

   Finish with the path and any unresolved product question. Writing the draft
   adds no approval stop and launches no implementation itself. Do not wait for
   optional review or treat silence as approval. Respect an explicitly requested
   review-before-build boundary; an unresolved choice remains unresolved.
