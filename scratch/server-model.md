# Server-first model: direction under discussion

November 10, 2026. Nothing here is chosen. `universal-server.md` remains the
reconciled plan for the narrower "shared observer" cut; this note records the
wider question raised after it and the reading that motivates it.

## What was said

Jack Heart, in the LOO-819 conversation:

- "Over the summer I redesigned Loopflow to make everything happen through independent processes. I took all the orchestration out of what was previously LFD and moved it into basically just having shared state and allowing processes to move that state forward in anyway"
- "I want to go back. I want to change the model so that we bite the bullet and say: 'Hey if we're going to install a shared backend server, auto-install it like OpenCode and Rex do anyway, then what does that imply about how the system should be designed?'"
- "if you're using Rex, Rex is already sort of maintaining your processes for you so Loopflow maybe doesn't need to maintain the live processes itself and then we can always have the Loopflow server do the same thing if you wanted to use the Loopflow desktop."
- "you might want to be able to move these sessions across from Rex to Loopflow Desktop but it's not necessarily the case that you need to preserve the exact same process"
- "It would be good to make Loopflow shrink Loopflow's service area to some degree so that it's modular."
- "allows your agents to have a good understanding of all the work that's going on in your machine and to separate that from some of the UI concerns that come from whatever specific terminal you're running it in"
- "Please help me figure out both the right overall long-term design and the first right deep slice to take on in order to explore this direction"

Later in the same conversation:

- "also see how were just leaking tons of processes every where"
- "and its basically opaque to me how many actual sessions are active in the background due to lf"

- "i guess my point is that for Loopflow Desktop, it seems to make sense to have loopflow own the store"
- "could we make it so that we actually make rex an dependency of loopflow and have rex own the terminal?"
- "potentially weird thing there would just be if you then open rex you see all of your loopflow stuff"
- "i think we should start with just doing Desktop does pty ownership for lf sessions"

Rex as installed here (`rex version`: 0.0.1793377904-ge6592f, built
2026-10-30, api revision 8; server pid up since 2026-10-31), from its help
text only, nothing exercised: `rex wait` "returns the child process's exit
status"; `rex events --json` streams session changes; `data.connect` issues a
ticket for a block's `rex.data.terminal.v1` stream; `session.new_block` creates
a block in no window; `rex server run --listen … --state-dir …` runs a separate
server; `--max-sessions` defaults to 64; built-in Tailscale listener.

A second count pasted with those remarks (source not stated) gave 314 lf
processes: 114 Session servers, 113 terminal-status observers, 45 Codex event
observers, 22 Claude Task-update watchers, 8 attach clients; 37 from test/demo
binary paths; "Of the 235 Session-related processes, 193 are over a day old."

Pasted with the first message (first person, source not stated):

- "There are things like having a hub that can receive webhooks or something to receive requests from mobile devices. They really want there to be one server."
- "I'm experiencing a lot of load contention on my machine as I scale up the number of LF processes. We really want to optimize Loopflow so that basically everything takes as little CPU and as little memory as possible."
- "Previously there have been a lot of dumb issues with things like codex hooks and observe hooks taking up 200% of the CPU, or with us trying to export the playing database while looping all the time."

## Live reading, this Machine, November 10 16:02

`ps -axo pcpu,rss,command`, one sample; `pcpu` is ps's decayed average. 16
cores, 128 GB, load averages 330.69 / 178.80 / 120.18.

| lf command | processes | CPU | RSS |
| --- | ---: | ---: | ---: |
| `session observe-status` | 109 | 0.0% | 3.2 GB |
| `session serve` | 95 | 55.2% | 4.4 GB |
| `__codex-observe` | 45 | 79.1% | 3.3 GB |
| `__context-block --moment task-updates` | 17 | 2.7% | 0.9 GB |
| `session attach` | 7 | 0.0% | 49 MB |
| `task retire` | 1 | 80.6% | 44 MB |
| `server run` | 1 | 0.0% | 53 MB |
| all lf | 281 | 218% | 11.6 GB |

About 45 `codex` processes were live. Resident lf processes per Session
(owner, relay or status pipe, Codex observer) account for 256 of the 281.

## Related earlier design

`../loopflow.icons/scratch/agent-in-terminal.md` (November 5, three questions
left open there): provider runs in the pane as the foreground process group,
no owner or relay, hooks fail soft, continuity by native resume.

## Superseded: Desktop holds the PTY

Set aside by "Direction as restated" below; kept for the code locations.

Proposal following the last remark above; scope not yet confirmed.

A Session shown in a Desktop pane runs its provider in that pane's Ghostty
PTY. No `session serve` owner, no `session attach` relay, no
`session observe-status` child.

- Launch: the command `lf resume --prepare --json` returns for Desktop takes
  `run_native_session`'s existing no-owner branch (`lf/commands/util.rs:597`)
  and skips `transport::delegate*` (`bin/lf.rs:2315`,
  `ops/human_session.rs:1086,1209,1306`). lf stays as the waiting parent and
  settles exit and capture, as that branch does now.
- Program Status: Desktop sends each pane's snapshot over its existing server
  connection as a typed write; `ProgramStatusObservation.start` stops spawning
  a helper per pane.
- Desktop state: no `detached` for these Sessions. A closed pane or quit app
  ends the provider; showing the pane again resumes natively. Restore resumes
  a pane when it is shown, not at launch. Move here ends the process where it
  runs, then resumes here.
- `ServerConnection` / `LF_SERVER_STATUS` (Opening…, Reconnecting…, Retry)
  has nothing left to report for these panes and goes with the relay.

Not in this slice: plain-terminal and Rex launches (owner unchanged),
`__codex-observe`, hook cost, and retiring the owners already running.

Demo: ten Sessions open in the Dev app; `ps` shows ten waiting `lf` and ten
providers, no `serve`, `attach` or `observe-status`. Quit the app: all twenty
exit and `lf session list` shows the Sessions ended. Relaunch: a pane resumes
when shown.

## Direction as restated

Jack Heart, after the Desktop-holds-the-PTY and Codex-observer proposals:

- "I think what you were proposing sounds slightly backwards to me what I was saying was we take all the PTY ownership code and move it out of the direct loopflow use case. We make bare LF runs skip any kind of and be a much more direct invocation of the children."
- "I still have a question about whether we still do process tracking for those things, whether you can still open those sessions, and whether we need to rethink the LF API (or maybe the fundamental launcher API as well as the general CLI API for interacting with existing sessions)"
- "ideally what I want is that you can still use Loopflow to launch skills and graphs. If you do that just generically through the command line, then you still basically get our context management code and you get our telemetry. But maybe you don't get anything that we think, if you were getting it through Rex, you could already get it through Rex basically"
- "And then we move to having a shared universal server. If you use the LF task API or wave API, or the like, for launching task and wave sessions (which, again, the API might need to slightly change), or if you go through desktop (which, of course, is only going to be opening those kinds of sessions as well), then those get maintained via the server"
- "Then I think the last thing after that is you just think about: what if you want to open one of those task sessions in Rex, for example?"
- "I'm not sure we want or need any hooks in the bare ELF either" (dictated; "ELF" read as "lf")
- On opening a bare Session that is live elsewhere: "I think what would be nice is maybe we don't require keeping the exact same agent process but we just allow you to resume this as a new process in the same session"
- "We already allow you to connect to... but we would treat those sessions similarly to how we treat current raw Claude and raw Codex invocations" (existing path: docs/sessions.md "Bring in one you started elsewhere")
- "Yeah I think steers and tusk updates do need hooks and those are clearly task-only so those can be on the managed sessions. I'm still a little unclear on what happens if you want to open a managed session in Rex or, I guess, any other terminal can we loan the PTY?"
- "In Rex you can have two windows open with the same session open and if you type in one, you see it on the other one, on the other screen. I think that's the behavior I'd kind of want"
- "Okay this sounds right. Let's start. Let's do some research into both the server protocols that T3 Code, Peradev, and Paseo are using, as well as the Rex server API, and see if there is anything that we need to keep in mind as we change this"
- "regarding the cost in Rex, can we still get that? If you do bare LF you get the agent and therefore you get the right icons but if you try to do a task session in Rex then you get LF as the process. Does that work?"
- On several old holders staying alive, each on the build that started its Sessions, with the server restarting freely beside them: "im ok with that"
- Size with several viewers: "Owner sets the size. I would say that we switch owners on viewer open close; any new viewer becomes the owner riht away?"
- Emulator in the server, holder keeps only recent raw bytes, repaint after a server restart: "Yea the second"
- Transcript as the single telemetry record with hooks as a wake-up, or hook-recorded turns kept as their own source: "no idea"
