---
description: Write the intended user documentation before implementation, ready to read and edit while work continues.
requires: a chosen direction
produces: user documentation edited in place, with paths shared and presentation in an attached Desktop
action_style: procedural
---
Write the docs someone will use when the change ships.

1. **Recover the chosen experience.** Read the request, settled decisions,
   prototypes and existing docs. Work from the chosen direction; do not reopen
   exploration or turn this into implementation planning. No preceding skill,
   Graph or Task is required. For a change with no user-facing documentation,
   say so briefly and finish without manufacturing a draft.

2. **Write the user docs in place.** Edit the actual README, guide, or reference
   files that will ship. Follow the project's documentation structure. Lead with
   a realistic usage example and its expected result. Explain the concepts,
   prerequisites, options, defaults, failures and recovery needed to use the
   change; include only what matters to this experience. For a library, show
   a call site; for a CLI, commands and output; for an app, actions and what
   appears. Write publishable documentation, not a press release, architecture
   plan or list of promises to document later. Keep the scope proportional to
   the change: one revised paragraph can be enough.

   The branch's docs describe intended behavior; do not claim it is implemented
   or released yet. Keep that status in the working plan and conversation,
   following any established unreleased-docs convention. Preserve saved edits
   and settled choices. Reuse sufficient documentation rather than rewriting
   it on every run. Scratch holds design notes, never a second copy of the docs.

3. **Read it as a user.** Can someone follow the example without knowing the
   implementation? Awkward explanations and surprising prerequisites are design
   feedback. Resolve details implied by the chosen direction. If writing exposes
   a consequential unsettled product choice, name the exact question in the
   working design and report it; do not invent acceptance. Keep any independent
   documentation useful and distinguish proposals from settled behavior.

4. **Present in an attached Desktop.** Run
   `lf open --host desktop --file <path>` for the actual docs; repeat `--file`
   to present several. This uses only an existing Loopflow Desktop Files view,
   with rendered Markdown/HTML and its draft/attention policy. It never launches
   Desktop. If Desktop is absent, unavailable or has no matching Files view,
   report the actual paths and continue. Never fall back to a terminal editor,
   host, browser or conversation, including from Rex, Herdr, tmux or cmux.
   Explicit user requests to open files in another host remain separate from
   this automatic presentation. Do not create a scratch wrapper.
   Record the changed documentation paths in the existing implementation plan
   so the next worker reads them from this checkout.

   In the conversation, link the files with a short invitation: “This is what's
   being built. Edit these docs or discuss changes while work continues.” Keep
   that invitation out of the published docs. Report a presentation failure
   honestly while leaving the edited docs available at their paths.

   Finish with the paths and any unresolved product question. Writing the docs
   adds no approval stop and launches no implementation itself. Do not wait for
   optional review or treat silence as approval. Respect an explicitly requested
   review-before-build boundary; an unresolved choice remains unresolved.
