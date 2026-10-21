# Collaboration review: getting started as the second person

Scenario: Dustin clones a repo Jack already runs with Loopflow (etude, manabot or loopflow) and wants to work beside Jack. Read-only review on 2026-10-21; the code is cited at the worktree's HEAD. Ordered by how much each finding would hurt Dustin.

What Dustin finds: etude's `.lf/config.yaml` has only `release:`, so it plans on Git. manabot and loopflow set `planning: {provider: linear}` and a `linear.team`. All three commit `.lf/wave/*/GOAL.md`.

## 1. Default Git planning is private to Jack, but the docs call it shared · needs Jack
- **Problem:** Dustin clones etude, runs `lf wave show` and `lf task list`, and sees none of Jack's Tasks or Projects. His Tasks stay invisible to Jack too.
- **Where:** README.md:11 and docs/index.md:11 ("The plan is shared through Linear or your Git repository"); docs/glossary.md:55 ("Where your Tasks and Projects are shared: a Git ref by default"); README.md:342-344; the website's "your tasks live in your project's GitHub repository".
- **Accurate?** No, for two people.
  - With no `planning:` set, the default is `Git{shared: None}`, which is a personal ref (rust/loopflow/src/config.rs:231-249; planning_git.rs:82).
  - Etude really publishes to `refs/loopflow/planning/users/9a0c1265-…` on origin (`lf planning status`; `git ls-remote origin 'refs/loopflow/*'`).
  - Dustin's first connection mints a different key, so he gets a different ref.
- **Useful?** The only hint is docs/lf.md:363-385 ("defaults to your Git ref … omit `shared` for …/users/<user-key>"). It never says this means nobody else sees it.
- **Fix:** say the default is personal everywhere it's mentioned, and add a "Plan with another person" block to docs/lf.md#choose-planning-synchronization:
  ```yaml
  # .lf/config.yaml — commit this so everyone on the repo shares one plan
  planning:
    provider: git
    shared: team
  ```
  > Without `shared`, each person plans on their own ref and sees only their own Tasks.
- **Needs Jack:**
  - Should a personal default stay the default once a repo has two people?
  - Does switching an existing repo to `shared: team` carry Jack's personal-ref Tasks over? docs/lf.md:405 says configuration "selects the repository's entire plan, including retained history". I did not verify that the existing records get published to the new ref.

## 2. No setup path for joining a repo that already uses Loopflow · clear fix, plus one needs Jack
- **Problem:** Dustin doesn't know what to run first when `.lf/` already exists.
- **Where:** docs/getting-started.md:24-31 (Setup Paths); README "Keep work moving".
- **Accurate?** Yes, but every row assumes you are starting fresh.
- **Useful?** No. What is shared (committed `.lf/config.yaml`, `.lf/wave/`, `.lf/flows/`, `.lf/workflows/`, `.lf/skills/`) versus what is his (`~/.lf/config.yaml`, accounts, routes, Machine, Wave Machine, Sessions) appears only scattered across config.md:21, lf.md:150-160 and lf.md:404-406.
- **Fix:** a new section, "Join a repository that already uses Loopflow":
  ```bash
  lf user                      # your name on claims and comments; set it if empty
  lf account connect claude    # your own agent login
  lf planning status           # which plan you are reading: shared ref, personal ref, or Linear
  lf wave list                 # the repo's Waves (committed under .lf/wave/)
  lf wave show                 # everyone's Tasks, if planning is shared
  ```
  Follow it with a two-column table:
  - **Shared, in the repo:** `.lf/config.yaml`, Waves, Flows, Workflows, Skills.
  - **Yours alone:** `~/.lf/config.yaml`, accounts and routes, Machines, which Machine runs your Wave work, Sessions, crons you install.
- **Needs Jack:** what `lf init` should do for a joiner; see 6.

## 3. Linear sign-in needs Jack's OAuth app secrets · needs Jack (product gap)
- **Problem:** in manabot or loopflow, `lf account connect linear` fails with "set LINEAR_CLIENT_ID and LINEAR_CLIENT_SECRET, or Doppler, to enable Linear OAuth".
- **Where:**
  - docs/getting-started.md:295 ("connect Linear with OAuth").
  - docs/lf.md:442-444 ("Linear planning needs a Linear login and repository Team").
  - The init skill says it, but no user doc does.
- **Accurate?** Partly. The credentials come from the environment or Doppler (rust/loopflow/src/provider_auth/mod.rs:61-62, 2912-2933), and Linear has no API-key fallback (mod.rs:126-133). No user-facing doc names the variables.
- **Useful?** No. Dustin also needs membership in the bound Linear Team, which no doc says.
- **Fix:**
  - Now: document both requirements (the OAuth app variables, and membership in the repo's Team).
  - Product: a published client, or a per-user API key.

## 4. Ownership uses your display name as your identity; docs say names grant nothing · needs Jack
- **Problem:** if Dustin has no Git `user.name`, every claim reads as his own, so he can rerun or overwrite Jack's Task. If he changes his name, he loses his own claims.
- **Where:** docs/config.md:27 ("Names describe people; they do not grant authority"); docs/lf.md:170-179 (the "yours / someone else's" table).
- **Accurate?** No. `is_mine` compares the claim's `person` string to `lf user`, and a missing name on either side counts as yours (rust/loopflow/src/store/sqlite/task_ownership.rs:199-203; the writer check is at :215 and :254).
- **Fix:**
  - Docs: "Your display name is how Loopflow tells your claims from a teammate's. Set a distinct name before claiming (`lf user`)."
  - Product: refuse an anonymous claim, or key claims on something stable.

## 5. "The Wave's ongoing conversation" is really one per person · clear fix
- **Where:** README.md:152 (`lf connect --wave designer # the Wave's ongoing conversation`); the homepage Sessions copy.
- **Accurate?** Partly. Sessions are Machine-local and "only planning travels" (docs/lf.md:419-421), so Jack and Dustin each get their own Wave conversation. Neither sees the other's history.
- **Fix:** "your ongoing conversation for the Wave". Add one line: teammates share the plan and comments, not conversations.

## 6. `lf init` misreads an existing repo · clear fix
- **Where:** rust/loopflow/src/builtins/ops/skill/init.md.
  - :61 lists Waves with `find wave …`, but authored Waves live in `.lf/wave/`. A joiner is told there are no Waves.
  - :107-111 lists agents as Codex, Claude and OpenCode, missing Pi.
  - It never checks whether planning is a personal ref (finding 1).
- **Fix:** change the path to `.lf/wave`, add Pi, and add a check: "Planning: personal Git ref (teammates can't see it) | shared ref | Linear" in the topology block.

## 7. Two people's `lf cron sync` runs the repo's crons twice · needs Jack
- **Problem:** loopflow's `.lf/config.yaml` commits `release-run` and `telemetry-daily`. If Dustin runs `lf cron sync`, his Machine schedules them too.
- **Where:** docs/lf.md:667-689 ("Reading or cloning the repository installs nothing").
- **Accurate?** Yes, but nothing warns about duplicates. Installation is per Machine, with no coordination across people.
- **Fix:**
  - Docs: "Crons in repo config run on every Machine that syncs them. One person installs shared jobs."
  - Product: decide whether a cron belongs to its Wave's owner.

## 8. A teammate's Task: what Dustin can and can't do · clear fix (partly needs verification)
- **Accurate parts:**
  - docs/lf.md:170-187 is right and useful: a later claim wins on your own Machine, and the earlier owner's Sessions get a steer, even under Git planning (task_ownership.rs:444-480).
- **Missing parts:**
  - Opening Jack's Task (`lf connect --task`, Desktop) shows Dustin none of Jack's conversation. He gets a fresh Session on his own Machine.
  - With Git planning his comments sync, but they don't steer Jack's agents. Only Linear comments do (finding from the earlier docs pass).
  - I didn't verify whether `lf connect --task` on someone else's claimed Task claims it.
- **Fix:** a short "Working on a teammate's Task" block: comment (steers Jack's agents only with Linear), claim (takes it over, Jack is told), or open your own Session (starts fresh).

## 9. Small accuracy and wording problems · clear fix
- docs/lf.md:129 links `#select-git-planning`, which doesn't exist. The heading is "Choose planning synchronization".
- docs/lf.md:387-390 ("This build implements the enrollment baseline below. LOO-412 replaces it…") is internal status. Dustin can't act on it, so remove it or move it to the architecture reference.
- docs/subscriptions.md:9 says it will "route each repository through them". Add that routes are Machine-local (`provider_routes` table) and never committed, so Jack's routing never reaches Dustin.
- manabot commits `agent: codex`. config.md:45-46 frames repo config as "should apply to everyone" but doesn't say what happens when a joiner lacks that harness. Repo config outranks personal config (config.md:58), so the only override is per run with `-a`. Say so, or let personal config win for `agent` (needs Jack).

## Out of scope, noted
The Mac app is useful to Dustin immediately only if the plan is shared (finding 1). Otherwise it shows his own empty plan beside the committed Waves.
