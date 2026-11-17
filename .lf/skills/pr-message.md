---
extends: builtin
---
## Command changes

`lf help --all` prints the public command tree. When the diff adds, removes or
renames a Clap command, compare that tree before and after the change and list
each added or removed command under **What changes**: "Adds `lf skill customize`",
"Removes `lf transport start`". Mention changed arguments only when they change
what someone types. For an added command, name the Task or design it delivers,
or say none was recorded. Keep this list even when the change looks minor.
