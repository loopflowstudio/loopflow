# LOO-554 questions and assumptions

- **OpenCode forwarding is in scope (Jack Heart, October 21).** Jack wants
  credential forwarding to work for possible customers, so build P8:
  `lf machine connect <m> opencode` forwards the stored OpenCode credential so
  paid models work on a remote machine. A Zen key has no identity endpoint;
  validate it some other honest way (e.g. one cheap authenticated call) and
  say what was and wasn't verified.
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
- **#1635 owns tracing and banners (resolved; merged to main as 173bb90880).**
  It sends tracing to `logs/lf.log` and deletes the diagnostics banners. This
  branch leaves those files alone; QA built with #1635 merged in.
- **Release state.** v0.14.14 (#1636) is published; baseline uses it via
  `install.sh`, fixes are verified with this branch's build on the machine.
- **Claude trust prompt per agent worktree (decided; moved to LOO-565).**
  Jack said yes to pre-accepting trust for lf-created worktrees. LOO-565 owns
  it; leave it out of this branch.
- **Codex forwarding proven (2026-10-22).** Round two connected Codex on a
  rented machine and a headless `lf -a codex` turn answered there; see the ledger.
- **Newcomer messages: fix both here (Jack Heart, October 21).** `install.sh`
  must not print the maintainer "Promotion preflight … VERDICT" block to
  newcomers. `lf machine connect m github` without `gh` on the machine must say
  plainly that `gh` is missing there and how to install it, rather than
  "machine credential command failed … inspect its account status". Prefer
  surfacing the real cause over adding a precheck guard.
- **OpenCode OAuth gets a fresh device grant; API keys are copied (decided
  2026-10-21, reversible).** Copying the laptop's active Console login would
  share one refresh grant across machines, which `docs/security.md` rules out
  for every other OAuth provider. So an OAuth login is minted fresh with
  `opencode auth login opencode --method device` and approved in the laptop's
  Chrome profile; an API key has no grant and is copied, like a GitHub token.
  Zen accepting the credential is the check; an API key's owner is not
  verified, and the success line says so.
- **Approvals were clicked by the run (2026-10-22).** cmux computer use was
  down, so the run used Chrome's AppleScript JavaScript in the tab lf opened
  (Codex account chooser + consent; OpenCode device "Authorize"), both in the
  jack@loopflow.studio profile. Jack asked for the Codex proof to be done
  without him, so the run completed those approvals itself; no Claude approval
  was needed in round two.
- **lf's local OpenCode integration is 1.x (follow-up Task, not here).**
  `extract_opencode_zen_token` reads `~/.local/share/opencode/auth.json`,
  which OpenCode 2 no longer writes; on Jack's laptop it finds a key Zen
  rejects (`api_key_expired`), `lf account` calls it "cached active", and
  `OpenCodeZenBroker` waits for that file after login. OpenCode ignores the
  injected key while it has its own login, so nothing breaks today. A Task
  should move lf's OpenCode account reads to `opencode auth export`.
- **OpenCode failure text before login (follow-up, not here).** A paid model
  with no OpenCode login on the machine makes `lf -b -a opencode` report
  "OpenCode native request: error sending request for url (…/prompt)" instead
  of OpenCode's own reason. Belongs to `harness/opencode.rs`; a separate Task.
- **Version skew from a Docker build (noted).** A branch build without git
  metadata reports `0.14.14` against the laptop's `0.14.14+<sha>`, and every
  machine command prints the skew line. Only affects ad-hoc builds.
