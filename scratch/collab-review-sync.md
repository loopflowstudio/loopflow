# Two people, one repo: staying in sync (docs review)

Scenario: Dustin works with Jack in one Loopflow repo (etude, manabot or loopflow).
Ordered by how much each gap would hurt Dustin.

## 1. With Git planning, Dustin never sees Jack's plan — needs Jack (and copy fix)
- **Problem:** Dustin clones, runs `lf task create`, and sees none of Jack's Tasks; Jack sees none of his.
- **Docs:** docs/lf.md:363-366 ("Planning defaults to your Git ref on origin"). README.md:11-12 and the homepage say "The plan is shared through Linear or your Git repository."
- **Accurate?** The lf.md text is literally true; README, docs index and homepage are wrong by default.
  - Without `shared:`, each person syncs only their own `refs/loopflow/planning/users/<key>` (rust/loopflow/src/ops/planning_sync.rs:95-113).
  - Binding refuses any other user key (store/sqlite/planning_peers.rs:121-131).
- **Useful?** Misframed: "personal by default" is never connected to "so set `shared:` before a second person joins".
- **Fix:**
  - lf.md: lead the section with "Working with someone? Commit `planning: {provider: git, shared: team}` to `.lf/config.yaml` so you both read one plan. Without `shared`, each person's plan is private to them."
  - README/index opening: "shared through Linear or a shared Git ref".
  - Product question for Jack: should a repository with a second contributor default to shared?

## 2. Comments steer agents only with Linear — needs Jack
- **Problem:** Dustin `lf comment`s Jack's Task; Jack's running agent never hears it.
- **Docs:** README.md:194-202 and docs/conducting.md:134-135 now state this. Homepage "Arrange" paragraph and README.md:12 imply it works everywhere; README:12 is already scoped to Linear.
- **Accurate?** Yes after the last pass. Git-imported comments are inserted with no steer (store/sqlite/planning_write.rs:444-461); steers come only from `append_task_comment` and Linear ingest (ops/linear_observe.rs).
- **Useful?** Dustin's real question is "how do I tell Jack's agent X?". With Git there is no answer today; the docs state the limit but offer no alternative.
- **Fix:** Product gap; filing "Git-imported comments become steers" makes Git and Linear equal. Until then, the homepage line should say "with Linear".
- Contrast: a later *claim* does steer the earlier claimant's Sessions on any transport (store/sqlite/task_ownership.rs:452-481), so the mechanism exists.

## 3. Both machines run the repository's crons — needs Jack
- **Problem:** `crons:` lives in committed `.lf/config.yaml`. If Dustin also runs `lf cron sync`, `daily-review` / `wave-operate` runs twice a day: duplicate Tasks and duplicate comments.
- **Docs:** docs/lf.md:668-690, docs/config.md:184-185, docs/waves.md:325-326 ("Your Machine for the Wave's Tasks runs its crons").
- **Accurate?** Yes: placement checks only this person's own Wave Machine (ops/cron.rs:611-619). Nothing coordinates across people.
- **Useful?** Missing: no line says "one person should install a Wave's crons".
- **Fix (clear, as doc):** after "Reading or cloning the repository installs nothing": "Crons are per person. When two people share a repository, choose one to run `lf cron sync` for each Wave, or give personal checks their own names with `lf cron add`."
- Product question for Jack: should a cron declare an owner?

## 4. No "refresh now" for Git planning — needs Jack (or clear doc)
- **Problem:** Jack says "I filed ETU-20". Dustin runs `lf wave show` and doesn't see it.
- **Docs:** docs/lf.md:419-421 ("Sessions and Desktop's Work connection keep planning current while connected. Cold Task lookup acquires planning").
- **Accurate?** Partly. Connected Sessions poll Git every 5s (ops/planning_sync.rs:403-407). Cold lookup of an exact ID acquires (planning_sync.rs:24-43). `lf task create` acquires (ops/task.rs:1493).
  - `lf refresh` is Linear-only (lf/mod.rs:1899 "Refresh shared planning from Linear"; ops/linear.rs:436).
  - Not verified: whether `lf wave show` acquires.
- **Useful?** Dustin needs "how fresh is what I'm looking at, and how do I update it".
- **Fix:** document "`lf task status ETU-20` fetches a Task you don't have yet; an open Session or Desktop keeps the rest current within seconds". Either make `lf refresh` cover Git, or say it's Linear-only.

## 5. Concurrent Task edits: conflicts exist, but nothing explains them — clear fix
- **Problem:** Both people retitle or re-scope a Task offline. What wins, and where does Dustin see it?
- **Docs:** docs/lf.md:419-424 ("Retained alternatives can be copied into ordinary edits"); `lf planning status` ("held records").
- **Accurate?** Yes as far as it goes (planning_peer_conflicts table; architecture-reference "Planning exchange").
- **Useful?** Missing: what a conflict looks like, which command shows it, and how to resolve it.
- **Fix:** add a short example to lf.md: two edits, `lf planning status` output showing the held alternative, then `lf task edit` to settle it.

## 6. Wave memory is a merge-conflict hotspot — needs Jack
- **Problem:** Both people's agents run `realign` and fold every PR into `.lf/wave/<w>/MEMORY.md`. Every second PR conflicts on the same file.
- **Docs:** docs/waves.md:282-303 ("ordinary reviewed repository file"); waves.md:505 ("Each shipped PR folds into memory").
- **Accurate?** Yes.
- **Useful?** Misframed for two people: no guidance on who curates, or how to keep edits mergeable.
- **Fix:** add guidance to waves.md "### Memory": "With two contributors, keep memory as short independent sections; an agent adds or edits one section per PR. Resolve conflicts like code." Jack decides whether memory curation should move to one owner or a non-PR path.

## 7. Wave and Task conversations are per person, but read as singular — clear fix
- **Problem:** Dustin runs `lf connect --wave runs` expecting Jack's Wave conversation and gets his own empty one. He also can't read what Jack's agents did beyond the PR.
- **Docs:** README.md:147-150 ("the Wave's ongoing conversation"); docs/waves.md:325-329 (Session Machine is personal); docs/lf.md:426-428 ("Only planning travels… Sessions, Processes… stay local").
- **Accurate?** Yes, but the reader has to assemble it.
- **Useful?** Missing a plain statement of what each person sees of the other's work:
  - Each person has their own Wave and Task conversations.
  - What crosses over is the plan, comments, ownership notes and pull requests.
  - Sessions, Flow history and usage stay on the machine that ran them.
- **Fix:** add a "Working with another person" section (best home: docs/conducting.md, or lf.md beside planning sync) listing exactly that.

## 8. Usage and cost are per machine — clear fix
- **Problem:** Jack runs `lf usage --task ETU-14` and sees only his own share, though Dustin did most of the work.
- **Docs:** docs/lf.md (`lf usage --task`), docs/subscriptions.md.
- **Accurate?** Yes. Usage comes from Machine-local Session history, and accounts are personal.
- **Useful?** Missing a caveat; the old /cli copy said "what the work cost, by Task or by Wave".
- **Fix:** one line beside `lf usage --task`: "Usage covers Sessions recorded on this Machine; a teammate's runs appear on theirs."

## 9. Process changes: team vs personal — clear fix
- **Problem:** Dustin wants to try a lighter `pursue` without changing Jack's.
- **Docs:** docs/config.md:582 ("Repository sources win over personal sources"); docs/authoring.md:30-33, 317.
- **Accurate?** Yes. A committed `.lf/skills` or `.lf/flows` definition overrides any personal one. Dustin can't shadow the team's version locally; he can only use a different name.
- **Useful?** Missing the consequence for two people.
- **Fix:** add to config.md: "To try your own version of a team skill, give it a new name in `~/.lf/skills` and run it by name. Changing the repository copy changes everyone's process; send it as a PR."
- Also check: a Wave Workflow stored with `lf wave workflow set --file` keeps its definition in local `wave_workflows`. Only the selected name travels in planning (store/sqlite/project_content.rs:94-106; work/edit.rs `WaveWorkflow`). Dustin may receive a Workflow name with no definition. Not verified end to end.

## 10. "Waiting on you" across people — needs Jack
- **Problem:** Jack's Task reaches a `design` node; Dustin was supposed to review it. How does Dustin find out?
- **Docs:** docs/authoring.md:260-304 (nodes are "where you take part in the Task conversation"); docs/conducting.md:85, 130 (`lf session list --available`).
- **Accurate?** Yes for the owner. A node has no notion of which person, and the conversation lives on the owner's machine.
- **Useful?** The homepage says "the owner reviews the design". A second-person review exists today only through the GitHub PR review.
- **Fix:** docs say "nodes are for the Task's owner; teammates review through the pull request". Whether a node can name another person is Jack's product call.
