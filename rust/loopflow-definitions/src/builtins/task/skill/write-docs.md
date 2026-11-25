---
description: Write the user docs before implementation, ready to read and edit while work continues.
requires: a chosen direction
produces: user docs edited in place, with their paths shared
action_style: procedural
---
Write the docs people will use when the change ships, before it is built.

Docs written first are the cheapest prototype: an awkward explanation or a
surprising prerequisite is design feedback.

1. **Start from the chosen direction.** Don't reopen exploration or plan the
   implementation. If the change has no user-facing docs, say so and finish.

2. **Write in place.** Edit the README, guide or reference that will ship,
   following the project's structure. Lead with a realistic example and its
   result: a call site for a library, commands and output for a CLI, actions
   and what appears for an app. Cover what this experience needs and nothing
   else. One revised paragraph can be enough. These are publishable docs, not
   a press release or a list of things to document later, and `scratch/` never
   holds a second copy.

   They describe intended behavior; don't say it is built or released. Keep
   edits people have saved, and reuse docs that are already good.

3. **Read it as a user.** Can someone follow the example without knowing the
   implementation? Settle details the direction implies. If writing exposes a
   real product choice, write the exact question into the plan and
   report it.

4. **Show it.** Run `lf open --file <path>`, repeating `--file` for several.
   It shows the files in an attached Files view and never launches anything;
   with none attached, report the paths and continue. Record the changed paths
   in the plan so the next step reads them.

Finish with the paths and any open question, and an invitation to edit the
docs or discuss them while work continues. Don't wait for a review nobody
asked for; silence is not approval.
