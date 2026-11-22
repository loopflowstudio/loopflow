Read when the change touches `yolo`, the permission flags or sandbox settings a launch sends, or what a vendor shows about them.

# Permission flags per launch

Stand-in providers that append their argv (and `$OPENCODE_CONFIG_CONTENT`) to
a log show what each launch sends, in a private `LF_HOME` and `HOME`. Exit at
once: a stand-in that reads stdin blocks a `-p` stream-json run forever. Grep
the log for `--dangerously-skip-permissions` (Claude, Antigravity),
`--sandbox danger-full-access` / `never` (Codex terminal), `"effect":"allow"`
and `--auto` (OpenCode). Attended = `script -q /dev/null lf -a <h> : '…'`;
unattended = `lf -b …`, `lf commit`, a Task Graph step (`-p` in argv; the
Task's primary conversation is attended).

Keep stand-ins in a directory apart from the `lf` that `LF_BIN` names: lf puts
`LF_BIN`'s directory first on the provider's PATH (`set_vendor_std_env`), so a
stand-in beside it wins over a real provider earlier on PATH.

Headless Codex sends its settings as `thread/start` params over app-server; a
stand-in cannot show them. Run real Codex (`CODEX_HOME=~/.codex`) and read the
rollout under `~/.codex/sessions/<date>/` that mentions the checkout:
`"sandbox_policy":{"type":"danger-full-access"}` and `"approval_policy":"never"`.

Real Claude in a PTY (pyte screen, answer `ESC[6n`): an attended launch in a
directory Claude has never trusted first shows "Adding a new directory" for
the `--add-dir` root, and the answer holds only for that Claude session; press
Down, Enter. The footer says `⏵⏵ bypass permissions on` or `⏸ manual mode on`.
This account's `~/.claude/settings.json` has `defaultMode: bypassPermissions`,
so `yolo: false` still shows bypass on. To see Claude ask, put
`{"permissions":{"defaultMode":"default"}}` in the checkout's
`.claude/settings.local.json`; it outranks the user file.

A personal `lf config set yolo false` in the main checkout reaches the
Session's `.agent` worktree; a committed `.lf/config.yaml` reaches only the
checkouts that have that commit.

Real `agy` in a TUI shows the setting by what a command reaches, not by a
footer: in a repository under the home directory, ask it to run `curl -sI
https://example.com | head -1` "with BypassSandbox false, do not retry". With
`yolo` unset the transcript (`brain/<conversation>/…/transcript_full.jsonl`)
holds `HTTP/2 200`; under `--setting yolo=false` the command runs in `agy`'s
sandbox, exits 0 and prints nothing. A first launch in a folder `agy` has not
seen shows "Do you trust the contents of this project?" whatever `yolo` says.
