# LOO-554 QA ledger

Machine: RunPod `tmyi9kd0pyjt68`, 2 vCPU / 4 GB, US-CA-2, $0.06/h, Ubuntu
24.04, user `qa`, tmux 3.4. (16- and 8-vCPU CPU shapes had no capacity on
2026-10-21; the 4 GB pod OOM-killed `cargo build --release`, so the branch was
built in Docker `linux/amd64` on the laptop and copied over.)
Harnesses: Claude Code 2.1.307, Codex 0.163.0, OpenCode 2.1.2
(`npm i -g @opencode/cli`), Pi 1.4.0. Baseline: published lf 0.14.14 via
`install.sh`. Branch: this branch merged with #1635 head e44ab917, run as
`~/lfb` (`LF_HOME=~/.lf-qa LF_BIN=~/lf-branch`).
Logins: GitHub and Claude (jack@loopflow.studio) forwarded with
`lf machine connect`; Claude approval was given in Chrome. Codex: approval
reached the machine once and failed there because `codex` was not on lf's remote PATH (fixed); two later requests ended
without a login, so Codex account checks are UNKNOWN.
OpenCode used its free Big Pickle model; Pi had no credential (Pi owns them).

Format: `<harness> <check> | command | result | evidence`

## Launch and output

- claude launch | `lf -a claude` (0.14.14) | FAIL→PASS: two `Loopflow diagnostics:` lines and `moved to` above the UI; branch+#1635 shows none | cl2-launch-0.14.14.txt, clb-launch-branch.txt
- opencode launch | `lf -a opencode` | FAIL→PASS: diagnostics line on 0.14.14; none on branch | oc1-launch-0.14.14.txt, ocb-launch-branch.txt
- pi launch | `lf -a pi` | FAIL→PASS: 0.14.14 exits instantly with only "Left “default”" (lf's Pi extension threw on missing `LF_BIN`); branch opens Pi with its own "No API key" message | pi1-launch-0.14.14.txt, pi-launch-branch.txt
- codex launch | `lf -a codex` (no login) | FAIL→PASS: every Codex launch on stock Ubuntu failed ("codex app-server exited before opening its socket"); lf's socket dir was 0775 under umask 002 and Codex refuses that. Branch: 0700 dir, Codex opens its own sign-in screen; early exits carry Codex's stderr line | codex-launch-branch.txt (before the socket fix)
- codex headless (no login) | `lf -b -a codex : …` | FAIL→PASS: "agent exited with code 1. Check <dir>" becomes a line naming Codex's 401 | —
- claude exit | decline trust prompt | FAIL→fixed: lf blamed "another client" (its message ended "… another client still owns it"); branch reports the exit and makes the takeover hint conditional | cl1-after-onboard.txt
- all headless redirected in tmux | `lf -b -c … > out` in a pane | FAIL→PASS: 0.14.14 wrote alt-screen/mouse resets and OSC 7501 around the answer; branch stdout is the answer only | —

## Per harness

- claude tool call + failing tool | `ls`, `cat README.md`, `cat missing-file` | PASS, native rendering, no lf lines | qa2-tools-0.14.14.txt
- claude detach/resume | kill tmux session, `lf resume <id>` from `qa2` | PASS, same conversation | qa2-resume-0.14.14.txt
- claude headless | `lf -b -a claude:sonnet : …` | PASS, stdout `PONG`, stderr empty, transcript `claude-sonnet-5-5` | —
- claude account | `/status` | PASS, Claude Max, jack@loopflow.studio, matches `lf account` | cl2-status.txt
- claude model across resume | `lf -a claude:sonnet`, `lf resume` | PASS, both turns `claude-sonnet-5-5` | cs3-resume-branch.txt
- claude flow | `lf -b -a claude:haiku flow qa2` (two skills) | PASS, stdout `STEP-A`/`STEP-B`, stderr flow progress only, both steps haiku | —
- claude/opencode env + argv + logs | `/proc/<pid>/environ` names; argv and `~/.lf/logs` prefix scan | PASS: no token names in Claude's env (P10 not seen); `opencode serve` holds its own `OPENCODE_PASSWORD`; 0 argv/log matches | —
- opencode paste in | tmux `paste-buffer -p` of 40 CJK/emoji/combining/tab lines | PASS, `[Pasted ~40 lines]`, nothing submitted early, stored turn contains payload byte-exact | oc2-pasted.txt
- opencode copy out | tmux copy-mode selection | PASS, no escape residue (side panel text comes along, as with any full-screen TUI); OpenCode's own OSC 52 copy not exercised → UNKNOWN | oc2-answer.txt
- opencode headless | `lf -b -a opencode : …` | PASS (3/4; one Big Pickle step ended without text and lf named the failure) | —
- pi paste in | tmux bracketed paste | PASS composer holds `[paste #1 +41 lines]`; byte check UNKNOWN (no Pi key, turn refused before persisting) | pi-paste-branch.txt
- pi headless | `lf -b -a pi : …` | PASS: stderr names Pi's "No API key" | —
- codex account | round two: `lf machine connect m codex jack@loopflow.studio`, then `lf -b -a codex : "Reply with exactly PONG"` on m | PASS: stdout `PONG`, stderr empty; `lf account codex` there lists jack@loopflow.studio active (verified), pro | r2-codex-proof.txt
- codex paste, tools, resume, model | — | UNKNOWN: round two proved the account only |
- claude paste in, copy out with OSC 52 | — | UNKNOWN: not run; time went to the Pi/Codex/transport defects |

## `lf -c`

- tmux buffer, xclip present, no display | `lf -b -c` in a pane | FAIL→PASS: 0.14.14 launched with no clipboard; branch delivered `CLIPTEST 漢字 🦀` | —
- X clipboard | `xvfb-run`, xclip set | PASS on branch | —
- no source | `lf -b -c` outside tmux, no display | FAIL→PASS: branch refuses before launch naming xclip, xsel, wl-paste reasons | —

## Newcomer path

- install | `curl … install.sh \| sh` | FAIL→fixed in code: the published promotion renders the preflight block only for `--preview`; visible from the next release's install.sh (a dev build cannot promote) | install.txt
- github forward | `lf machine connect m github` without `gh` | FAIL→PASS (round two): names "GitHub CLI (gh) is not installed on this machine; install it (https://cli.github.com)…"; after `apt install gh`, "Connected github on m" | r2-gh-missing.txt
- version skew | `lf machine add` laptop 0.14.12 → remote 0.14.14 | FAIL→fixed: advice told you to update the newer remote |
- free-text message | `lf -b -a opencode "message"` | FAIL→fixed: not-found error names `lf : <message>` |
- remote -a pi | P5 | fixed in code, unit test; not exercised on the machine |
- OpenCode hint | P9 | fixed: hint names `@opencode/cli` (2.1.2) |

## OpenCode forwarding (round two)

- paid model before connect | `lf -b -a opencode:opencode/gpt-5.4-nano : …` on m | FAIL as expected, but lf's text is "execution failed: invalid data: OpenCode native request: error sending request for url (…/prompt)", not OpenCode's reason (follow-up in questions.md) | —
- connect | `lf machine connect m opencode --chrome-profile 'Profile 7'` | PASS: fresh device login approved in the jack@loopflow.studio profile; "Connected opencodezen on m: Zen accepted a fresh login for the same OpenCode account as this laptop" (the slug on that line is fixed at 00deba37ad: both lines now say "OpenCode Zen"; not re-run on a machine) | r2-opencode-connect.txt
- paid model after connect | same command | PASS: stdout `PONG`, stderr empty | —
- second connect | same connect again | PASS: 1.9 s, no browser, nothing transferred | —
- laptop login | `opencode auth list` before/after | PASS: unchanged | —
- secrets | access/refresh bytes grepped under m's `~/.lf-qa` (non-db) and `logs` for oauth/bearer | PASS: 0 files | —
- found on the machine: `opencode auth export opencode` waits forever when that integration has no entry; lf now exports all and filters `integrationID` (fixed, re-run above)

## Spend

Pod `tmyi9kd0pyjt68` 2026-10-21T22:53Z → deleted 2026-10-22T00:34Z (HTTP 204;
pod listing shows only the unrelated manabot pod). 1h41m at $0.06/h ≈ $0.10,
well under the $4.30 left of Jack's $5 cap.

Round two: pod `oou1q5x1h8tccw` 2026-10-22T02:22Z → deleted 02:44Z (HTTP 204,
then 404 "pod not found"); 22 min at $0.06/h ≈ $0.02. Total ≈ $0.12.
The Codex and OpenCode grants minted for that pod went with it; neither
provider was asked to revoke them.
