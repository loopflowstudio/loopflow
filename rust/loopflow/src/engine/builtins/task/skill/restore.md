---
description: Restore the specific broken surface before investigating systemic causes.
action_style: procedural
---
Restore the reported surface.

Read the Task report and inspect the machine, logs, code, and current state it
names. Reproduce the failure when that is safe and useful. Keep observations
separate from hypotheses in `scratch/<branch>.md`.

Make the smallest safe change that gets this exact surface working again: the
reporter's machine, the server, or the release path. Migration failures are
recovery work, not permission to discard state. Preserve evidence needed for
root-cause analysis.

Prove restoration through the real reported path. Record the command or
observation that distinguishes recovery from a plausible workaround. Do not
stop at diagnosis, and do not broaden into systemic prevention yet; the next
step owns the 5 Whys.
