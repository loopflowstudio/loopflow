---
description: Write the user docs before implementation, ready to read and edit while work continues.
requires: a chosen direction
produces: user docs edited in place, with their paths shared and shown in an attached Desktop
action_style: procedural
---
Write the docs people will use when the change ships.

1. **Start from the chosen direction.** Read the request, settled decisions,
   prototypes and existing docs. Don't reopen exploration or plan the
   implementation. If the change has no user-facing docs, say so and finish.

2. **Write the docs in place.** Edit the README, guide or reference files that
   will ship, following the project's structure. Lead with a realistic example
   and its result: a call site for a library, commands and output for a CLI,
   actions and what appears for an app. Cover the concepts, prerequisites,
   options, defaults, failures and recovery this experience needs, and nothing
   else. Publishable docs, not a press release, architecture plan or list of
   things to document later. One revised paragraph can be enough.

   These docs describe intended behavior; don't say it's built or released.
   Track that status in the plan and conversation, or follow the repo's
   unreleased-docs convention. Keep saved edits and settled choices, and reuse
   docs that are already good enough. `scratch/` holds design notes, never a
   second copy of the docs.

3. **Read it as a user.** Can someone follow the example without knowing the
   implementation? Awkward explanations and surprising prerequisites are design
   feedback. Settle details the chosen direction implies. If writing exposes a
   real open product choice, write the exact question into the working design
   and report it. Don't write an answer into the design as settled unless
   someone said so; quote what they said.

4. **Show it in an attached Desktop.** Run `lf open --host desktop --file <path>`
   for the docs; repeat `--file` for several. It uses an existing Loopflow
   Desktop Files view and never launches Desktop. If there is none, report the
   paths and continue. Never fall back to a terminal editor, browser or the
   conversation, including from Rex, Herdr, tmux or cmux, unless that editor was
   asked for. No scratch wrapper. Record the changed doc paths in the existing
   implementation plan so the next step reads them from this checkout.

   In the conversation, link the files with a short invitation: “This is what's
   being built. Edit these docs or discuss changes while work continues.” Keep
   that line out of the docs. If presenting fails, say so; the docs stay at
   their paths.

Finish with the paths and any open product question. Writing docs adds no
approval stop and launches no implementation. Don't wait for optional review,
and silence is not approval. If a review before building was asked for, honor
it; an open choice stays open.
