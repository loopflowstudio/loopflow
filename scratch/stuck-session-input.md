# Stuck terminal input

2026-11-14, live LOO-904 investigation.

Report: “I often get int o a situation where lf sessions stop responding to input whether in Rex or in Desktop, e..g LOO-904 right now”.

Session `e9a123c0c9ed4f1291ec5a8e258fb715`, provider thread `01a1dcb0-e04e-79f0-a408-ae2fa94bb6cd`, AgentProcess `0a0be6c4-dc83-4b12-b7d9-e1ae89e1adb7` (pid 835), held terminal `b30e03b7-e04b-4e20-b9b0-44c2e22343c8` (Codex TUI pid 91512). Holder pid 83278, build `1bfe6d149449caedb31bf8e0846843a5be015a275875719e24406953f96fc3a9`. Server log reported `0.15.3+f2a581a2a`; checkout HEAD `3fbf3d2ca3`.

The Session card reported live, active, Rex control holder. Last provider turn ended at 2026-11-15T02:26:22.509Z. Holder ring stopped at byte 2739443. Retained draft contained `[Image #1] another issue is what are these New Conversation things`.

Direct holder input probes: left/right cursor keys, then space and backspace, produced no PTY output. Slave `/dev/ttys082` was raw, with zero unread input bytes. TUI process sample showed crossterm waiting in `UnixInternalEventSource::try_read` / `kevent`; app-server was idle. Samples are `/tmp/lf-stuck-91512.sample`, `/tmp/lf-stuck-835.sample`, `/tmp/lf-stuck-holder.sample` on this Machine.

Sending only `ESC [ 201 ~` directly through the holder write connection immediately produced PTY output and exposed accumulated typing/newlines in the draft, including “it does seem a little bett” and test-like letters. No Return or message was submitted by this investigation. This establishes an unfinished bracketed paste in the native terminal parser. The cursor/space/backspace probes may be included in the recovered paste; review the draft before submitting.

Still to determine: which transition lost the matching paste-end marker in this occurrence. The relay discards stdin during reconnect/catch-up and treats any Ctrl+] in a read as detach, including pasted content. Hosted input has no bracketed-paste lifecycle tracking when a controller leaves or is replaced. These are investigation leads, not established causes of this occurrence.

Second report: “515 is stuck there too now”. Session `1a2185651fce4f078f44b2201f49d660`, holder `ee0657220d1a`, terminal `a984705b-28f8-4076-a835-485cb72b1167`. Its ring stood at 5360465 bytes. A direct paste-end marker produced no new output. While the cursor probe ran, its Codex processes 39821/45748 disappeared and its Session changed to Claude AgentProcess `017d9c48-0fab-4532-b18e-3991d560183c`; card reported blocked login. No same-cause conclusion was possible. No message was submitted.

Baseline regression `an_interrupted_paste_ends_before_the_next_view_types` failed waiting for the closing paste marker; the child remained alive on a raw PTY. After the repair, all nine paste regressions passed. The first repaired run exposed a fixture output issue: raw-mode newlines did not reset the column, wrapping `closed` across rows in a snapshot; explicit carriage returns fixed the fixture.

Later read of LOO-515’s new Claude terminal `d9556792-3fe0-4956-974a-c81c92bbdde0` showed a completed response and draft `1 hybrid, 2c, build this` at its prompt. No input was sent to this new terminal.
