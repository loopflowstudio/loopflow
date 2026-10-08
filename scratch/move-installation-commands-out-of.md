# LOO-401 gate after main integration

Jack Heart selected `self` for installation and doctor, `config user` for the
display name, and removal of screenshot plus its hidden supervisor without
aliases. The implementation uses root `open` for the app. The accepted design and prior review remain at
`48eb77ad4:scratch/move-installation-commands-out-of.md`.

Main's #1489 now owns named Machine connections and global `--machine` dispatch;
Machine lists id/add/list/status/rename/remove. Gate removed the remaining Flow
rewrite to the deleted `machine ssh` path. Installation recovery dispatch,
short install/doctor/user lookup, schedules and Desktop's independent snapshot
remain intact. Current docs, generated reference and agent capture instructions
match the command tree.

Infrastructure memory fits its 16,000-token budget after archiving completed
LOO-342 detail at `173d649cf:wave/infrastructure/MEMORY.md`. Release's child memory
was read; its operation-entry lesson remains covered by removal and installation
entry proofs. No new product decision was needed.

Automated gate verification is complete; exact commands and the nested-sandbox
failure with its isolated passing check are in checks.md. No code blocker remains.
Native app launch and installed release acceptance were not exercised. Publication
and subsequent workflow navigation remain with the caller.
