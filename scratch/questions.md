# Open choices (LOO-926)

- PR 1 builds on `main`, not on LOO-913's `jack-heart/cli-prox` (#1953). That
  branch is still at `606c13c2d7`, before the crate split. Whichever lands
  second removes the `prepare_launch` calls in `write::Remote::call` and the
  CLI's `ssh::run`.
- From PR 2, `lf --machine X <command>` fails when the command has no typed
  `Operation`, and the error names `ssh X lf …`. The alternative is to keep
  argv pass-through for those commands as a terminal launch. Today
  `lf --machine X install server` (`docs/troubleshooting.md:166`) is the only
  documented case found.
- The remote entry is a visible `lf server stdio`, not a hidden `__op`. An
  older lf exits 0 on any unknown `__*`, which would hide a version mismatch.
- Typed calls skip the up-front SSH probe (and its "lf versions differ"
  line); the destination checks identity, and the version is read only when
  it exits 2. The alternative keeps one extra SSH round trip per call.

## Direction after LOO-913 landed (#1953, 2026-11-16)

Jack, on the `prepare_launch` and `--machine` items above: "This was supposed
to also do credential forwarding".

- LOO-913 replaced `credentials::prepare_launch` with `machine::forward`
  (`forward::read` + `forward::deliver`, from `forward.env`/`forward.files`).
  On main it is called from two SSH sequences: `lf/commands/ssh.rs:123,332`
  and `machine/task.rs:228,271`.
- The one SSH launch this Task builds carries `forward::deliver`: typed
  operations on `lf server stdio` and terminal launches both get the person's
  named credentials. No `prepare_launch` survives the sync.
- A bare `ssh X lf …` carries no forwarding, so telling people to type it for
  commands with no typed `Operation` drops their credentials. That choice is
  reopened: those commands keep a forwarded terminal launch through the same
  SSH launch.

## Concept review answers (2026-11-17)

Page: `scratch/concept-review.html`. Jack's reply, whole: "1a 2a 3a 4a 5c".

1a. SSH is a carrier for the one protocol, plus plain commands for install.
2a. When the destination's server is running, `lf server stdio` may hand its
    connection to it: subscriptions and conversations over SSH.
3a. Another Machine's server is not required for reads, writes and headless
    runs; it is for conversations. Said in one place.
4a. Forwarded logins travel with the SSH carrier only, once per connection. A
    Machine reached by WebSocket uses its own logins.
5c. Stop the stack here and redesign around the carrier first.

## Logins (2026-11-17)

Jack, after the table of what must be on a Machine and what forwards:

- "does this make sense? agent provider credentials are the most important
  [and most complicated to get right + legal] and should be the most
  important", then: "most supported".
- "i guess we maybe want to join in the lf machine connect logic into this
  concept review".

The review's logins section and questions 7–11 are in
`scratch/concept-review.html`. Unanswered: 6 and 7–11.

## Provider logins in sandboxes (2026-11-17)

Jack: "using an agent inside a sandbox is an extremely common use case. i am
confident the providers are ok with that. we should figure out how people are
doing this with companies like daytona and do the normal thing + make it as
convenient and automatic as we can without claerly crossing some sort of TOS
line". Then: "its ok if it works differently on a per-provider basis".

Read 2026-11-17 (quotes in `scratch/concept-review.html`):

- code.claude.com/docs/en/legal-and-compliance: hosting unmodified Claude Code
  in sandboxes is allowed; each end user signs in with their own credentials;
  developers "may not collect, store, or intermediate Claude.ai credentials or
  session tokens — sign-in to a Claude account must complete through
  Anthropic's own flow".
- code.claude.com/docs/en/authentication: `claude setup-token` makes a
  one-year token for `CLAUDE_CODE_OAUTH_TOKEN`, "for CI pipelines, scripts, or
  other environments where interactive browser login isn't available"; model
  requests only; bare mode does not read it.
- learn.chatgpt.com/docs/auth: headless prefers `codex login --device-auth`
  (beta, enabled in ChatGPT security settings); fallback is copying
  `~/.codex/auth.json`.
- learn.chatgpt.com/docs/auth/ci-cd-auth: "Do not share the same file across
  concurrent jobs or multiple machines."; ephemeral runners restore, run and
  write the updated file back.
- Daytona's guide signs in over SSH with the URL and pasted code; Daytona SDK
  examples, E2B and Docker cloud sandboxes use `ANTHROPIC_API_KEY`.

Unanswered: 6, 8–14 (12–14 replace 7).

## Logins Task

Filed 2026-11-17 as LOO-957, based on `main`, not started. Question 11 is settled as its own Task; 12–14 are open decisions in its brief.

- On several unattended Codex rentals at once, Jack: "running a login flow is fine if it can be automated and headless; this hsould be possible even if we have to invoke some sort of agent browser interface". Recorded on LOO-957.

- On Chrome profiles for approving logins, Jack: "This is a little awkward now because we need per-account profiles (thats basicalyl why profiles were introduced) but lf no longer knows about your different accounts". Recorded on LOO-957.

- Jack: "were also going to need something to forward new credentials when an account runs out". Recorded on LOO-957.
- On "lf owns one action: replace the login on a Machine …", Jack: "yeah good".
- On the helper: "lf can always just use the current login, and its outside lf's purview to replace the token, but it should capable of sending a replaced token if the current account is switched".
- "We should also make sure that we are initially capable of sending multiple provider tokens, so that there is osme failback between providers without the laptop sending new creds". All three recorded on LOO-957.
- Correcting "every provider you're signed in to", Jack: "well, all logins that are considered approved agent for the work request, as in compliance with our agents/default_agents config that pulls in values from perrsonal config, repo, wave, command line etc". Recorded on LOO-957.
- On learning of a switch, Jack: "im imagining something like a "resend token for all failed machines" CLI or whatever that is roughly manual for now".
- On the current login: "to be clear im not saying that it has to be the current login's actual keys. just that the current login should determine which accoutn generates the key. im not exactly sure how this works." Both recorded on LOO-957, with what `claude auth status --json` and `codex login status` report.
- Reversing the agent-selection rule, Jack: "actually hmm thinking about this , this is a bit wrong, you might still have a task which is only configured to run on claude and tell it "hey run this machine"" and "I think machine launches should just have a configuration of their own, that defaults to all your providers, for which provdiers to share with the machine". Recorded on LOO-957.
- Jack: "Ok great so then this make the retry bit even easier, it just retries and it doesnt even strictly require ive changed my current login (though i would likely have wanted to if we hit limits)".
- Jack: "The question becomes if loopflow isnt keeping track of multiple accounts, how does it route into the correct browser. we need some way of setting it up so that loopflow just opens a browser and runs the flow in some sort of automated way (either for the human to approve or in some sort of managed agent browser) and some way for someting like subpersona to steer which profile is launched". Both recorded on LOO-957.
- On where a rental's sharing setting lives, Jack: "Yeah, either in the launch settnig and persisted for later or the pool itself". Recorded on LOO-957.
- On a Machine with its own login, Jack: "yeah thats right, machines should be able to assert their own logins probably globally OR on a per-provider basis". Recorded on LOO-957.
- On the approver, Jack: "ok, i like the idea of just allowing lf to have a per-provider chrome profile [or someting slightly more generic? maybe command could consume both?] that it uses to do ideally both "login into X" or "login into X for this machine", and then subpersona's next could switch to changing this setting in LF, and then actually driving the login through lf (or not, not important)". Recorded on LOO-957.
- Jack: "wonder if this shoudl be lf be lf home = lf machine localhost basically". Recorded on LOO-957.
- Jack, on the same idea: "and reduce the API surface area". Recorded on LOO-957.
- Jack, clarifying: "[machine doesnt have to literally accept localhost, just mean conceptually]". Recorded on LOO-957.
- On "leaving the Machine out means here: lf machine connect claude", Jack: "yeah i like this".
- On disconnect: "we can either leave it out, or make it something we support for remote decisons". Both recorded on LOO-957.
- On folding `lf account` into the Machine commands despite the wide reach, Jack: "i am ok with this". Recorded on LOO-957.
- On one PR or two for LOO-957, Jack: "one pr". Recorded on LOO-957.

## Question 6 (2026-11-17)

Jack, after the trade-offs and "i dont even know what stdio is" / "why is the only way in ssh instead of websockets?": "to me it seems like if youre basically saying i want to proxy my commands to the remote server, then probably doing more work on the remote server is mroe expected".

Reading: the destination's resident server does the work; the SSH entry is a pipe to it. Not yet confirmed: that the destination's server becomes required for SSH-routed work (started on demand), which changes the brief's constraint "SSH routing needs no resident API server on the destination" and answer 3a.

- On work sent to another Machine needing its server, Jack: "not requiring machine server for simple ssh: this is a nice-to-have, but im open to droppign it if it simplifies behavior and/or infrastructure". Then: "there is always plain old ssh lf".
- On a Mac WebSocket client, Jack asked: "Would adding one simplify the system, or does it just ad more cases?" Finding: with SSH a pipe to the server it adds a second route with no new ability; taken out of the plan.

Settled by these: the destination's resident server runs every routed operation (question 6: the server does the work); the SSH entry is a pipe that starts the server when needed; answer 3a and the brief's "SSH routing needs no resident API server on the destination" are dropped; `ssh <machine> lf …` typed by hand is the way in when a server cannot run.
- Jack, confirming: "think its fine for the lf-owned ssh transport to require some lf infrastructure and in exchange give more magical behavior".
- On the server-does-the-work redesign, Jack: "ok. lets keep moving. i accept this new direction".

## PR 1 rework choices (2026-11-17, headless)

- The write another Machine sends is a new operation, `machine.write`
  (`{repository, environment, forwarded, change}`), not a field on `Request`
  or `hello`: additive, so Desktop, the phone and older servers are
  untouched, and an older server answers "no operation `machine.write`".
  Settles the open "repository id on Request vs entry rewrites `directory`"
  and "per-run secrets": the one-time file `forward::deliver` writes is named
  in the operation and read once by the destination's server
  (`forward::take`). The operation, not `hello`'s kind, marks a write that
  arrived; the kind `machine` is still sent, for `lf server status`.
- `task.ship` forwards what `forward` names, as `task.run` does: both start
  agents there. #1956 forwarded for runs only.
- The entry kept #1956's error texts ("mini runs lf 0.15.2 …", "lf on mini
  exited with status 101 before answering; the destination said: …"), now
  inside the error itself so a served request keeps them.

## Concept review of the built PR (2026-11-17, `60895c2e30`)

`scratch/concept-review.html` has the page. Its questions and Jack's answer, "1a 2a 3a 4a 5a":

1. Where "who is asking, for which repository" lives. **a**: on the request, typed, in this PR; `machine.write` and the environment map deleted before they ship.
2. What makes the server run an operation here and never send it on. **a**: the connection, a client that said it is a Machine; applies to every operation.
3. How a run's secret values reach the destination's server. **a**: in the request, as a field nothing can log; files still delivered as files.
4. A destination that accepts SSH and never answers. **a**: bounded in this PR, with a message naming the Machine.
5. Reads and `--subscribe` for a Task on another Machine. **a**: in this PR.

This reverses the headless choice above ("The write another Machine sends is a new operation, `machine.write`").

## Sender rework choices (2026-11-17, headless)

- `repository`, `selection` and `secrets` travel as one required nullable
  `sender` object, not three top-level fields: a client on this Machine sends
  one null, and they are present or absent together.
- `subscribe` carries `sender` too: a Machine's reads need its repository's
  checkout as much as its writes.
- `Selection` also carries `settings` (`LF_SETTINGS`, command-line settings),
  which the environment map forwarded.
- A `sender` from a connection that is not a Machine is ignored, not refused.
- The SSH answer bound is 120 s; the SSH handshake uses the same bound, since
  the entry may start the server first. Over the bound a request is
  `outcome_unknown`, a handshake `unavailable`.
- `lf task status --subscribe` for a Task another Machine runs prints that
  server's frames, keeps `--select`, and ends when stdin closes (after its
  first reading), as the local one does. It ignores stdin requests
  (`{"id":…,"args":…}`), which the local `--subscribe` answers: forwarding
  checkpoints needs a write half of `Connection` beside a blocked read (QA,
  2026-11-17).
- QA (2026-11-17): one-shot `lf task status <issue>` and `--json`, and served
  `task.status` requests, still read this Machine's copy of a Task another
  Machine runs; only `--subscribe` reads there. Answer 5 ("Reads and
  `--subscribe` … in this PR") covers them; left for the next implement pass,
  since the human output is built from the local `TaskStatus`, not the
  served `task.status` body.

## Sync onto main #1960 (2026-11-17)

- Main moved the protocol to revision 7 (`request_id` on `request`). `sender` on `request`/`subscribe` is revision 8 with `MIN_REVISION` 8, so a side built from main answers `incompatible` at `hello` instead of failing to parse.
