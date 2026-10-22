# LOO-554 questions and assumptions

- **OpenCode forwarding deferred (decided 2026-10-21, reversible).** P8 is not
  built. On the machine OpenCode 2.1.2 ran on its free Big Pickle model with no
  credential, Pi cannot use lf's key (Pi owns credentials), and an OpenCode Zen
  key has no identity endpoint, unlike every login `lf machine connect` forwards
  today. A Task can add it if Jack wants paid Zen models on remote machines.
- **Pi credentials stay Pi's (assumed).** `harness/pi.rs` says Pi owns models
  and credentials; lf has no Pi credential to forward. QA records what a
  newcomer sees on the rented machine (Pi `/login` URL copied out of tmux) and
  fixes only lf's messaging. Whether lf should forward Claude/Codex logins
  into Pi is Jack's product call; Anthropic's terms for subscription OAuth in
  third-party harnesses matter there.
- **Latest of every harness; OpenCode 2 for sure (Jack Heart, October 21;
  resolved).** OpenCode 2 is published on npm as `@opencode/cli` (2.1.2 on
  2026-10-21, newer than the laptop's 2.0.32), so no tag build was needed;
  `opencode-ai` stops at 1.19.3. QA ran Claude 2.1.307, Codex 0.163.0,
  OpenCode 2.1.2 and Pi 1.4.0 (`@earendil-works/pi-coding-agent`), each the
  npm latest on 2026-10-21. lf's OpenCode hint names `@opencode/cli`.
- **#1635 owns tracing and banners (resolved 2026-10-21).** Its head
  e44ab917 sends tracing to `logs/lf.log` and deletes the diagnostics banners.
  This branch leaves those files alone; QA builds with #1635 merged in.
- **Release state.** v0.14.14 (#1636) is published; baseline uses it via
  `install.sh`, fixes are verified with this branch's build on the machine.
- **Claude trust prompt per agent worktree (open, for Jack).** Each `lf`
  launch while earlier Sessions live creates a fresh `<repo>.agent-<hash>`
  worktree, and Claude asks "do you trust this folder" for each new path; its
  default answer exits. lf grants no trust today (`harness/context.rs`: "No
  user files or trust decisions are replaced"). Pre-accepting trust for
  lf-created worktrees is a product/security call.
- **Codex account checks UNKNOWN.** One Codex approval reached the machine and
  failed on lf's remote PATH (fixed); two later requests ended without a login.
  Demo step: `lf machine connect <m> codex jack@loopflow.studio`, then
  `lf -a codex` and `/status` on the machine.
- **Newcomer messages recorded, not fixed.** `install.sh` prints a maintainer
  "Promotion preflight … VERDICT" block; `lf machine connect m github` without
  `gh` on the machine fails with "machine credential command failed … inspect
  its account status" (remote output is withheld by design; a precheck would
  be a guard). Both are candidates for separate Tasks.
