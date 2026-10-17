# LOO-415 transport handoff — integrated October 17

PR #1581 at `2523534b29e465f68bec88989bb684899f3ca660` is the LOO-416 stack parent;
`7bfa10fa72` integrates it. Full handoff: [archive](evidence/loo-415-transport-handoff-stack.txt).
Use `rust/loopflow/src/transport/README.md` and the actual code as the contract.

Every execution has an independent on-demand owner, including native clients and
Flow steps. Execution delegates before capture admission so viewer loss cannot
settle accepted work. `LfSession.transport_lf_process_id` names the owner; viewer
control uses fresh runtime tokens, separate from provider attachment. Session attach
never starts/replays/transitions; missing owner remains unknown. Acknowledgement
has a five-second deadline, established attachments do not. Shells use Process IDs.

Commands: `lf session attach SESSION`, `lf transport attach PROCESS`,
`lf transport start --terminal -- COMMAND`; remote global `--machine LABEL`.
Viewer closure detaches; the owner drains zero-viewer output. vt100 retains a text
grid and 2,000 scrollback rows with supported modes. Images, hyperlinks, keyboard
extensions and exact restoration of both buffers are unsupported; don't claim exact
terminal recovery. Native drafts remain in the live client.

Eight transport fixtures, fmt/clippy and a focused resume regression passed upstream.
Configured providers, Desktop, nested CLI, twelve live Sessions, broader mode/remote
acceptance remain unproved. LOO-416 consumes transport, never starts a second holder.
