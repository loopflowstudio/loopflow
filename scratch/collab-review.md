# Two people, one repo: docs review (LOO-550, 2026-10-21)

Jack asked for a fuller docs pass imagining one other person (Dustin) working in
etude, manabot or loopflow. Source findings: collab-review-onboarding.md,
collab-review-dividing.md, collab-review-sync.md (code evidence there).

## Applied
- docs/lf.md: new "Work with another person" (shared-ref config, commands, what
  travels vs stays local, names, branches, takeover, operators, review, comments,
  crons, process, memory, Linear OAuth); planning section leads with "personal by
  default"; LOO-412 status note removed; broken #select-git-planning anchor fixed;
  usage-is-per-Machine line; branch naming; cron duplicate warning.
- README/index opening: "shared through Linear or a shared Git ref".
- README: `lf connect --wave` is "your ongoing conversation for the Wave".
- docs/config.md: names distinguish claims; team vs personal Skill variants.
- docs/getting-started.md: "Join a repository" setup path; Linear OAuth needs.
- docs/waves.md: memory as independent sections with two contributors.
- init skill: `.lf/wave` path (was `wave/`), Pi listed.
- Homepage: "shared ref" in the plan sentence.

## Needs Jack (product)
1. Default Git planning is a personal ref; etude plans on Jack's. Should a
   repository default to a shared ref? Does switching carry existing Tasks over?
2. wave-operate/repo-operate act on a teammate's owned Task, which looks idle
   from another Machine; `lf task run` then claims it. Add a "theirs" disposition
   and report teammate execution as unknown?
3. Git-synced comments don't become steers (Linear ones do). Homepage still
   claims "Comment on someone's task, and their agents read it".
4. Takeover under Git planning yields a second branch and second PR.
5. Workflow nodes belong to the owner's conversation; no in-Loopflow path for a
   teammate's review. Should a node name its reviewer?
6. Repo crons run on every syncing Machine. Should a cron have an owner?
7. Ownership keys on display names; anonymous claims read as everyone's.
8. Linear sign-in needs Jack's OAuth app secrets; no published client/API key.
9. `lf wave show` has no owner column; its "wave" column reads like an owner.
10. `lf refresh` is Linear-only; Git has no "fetch now".
11. Wave Workflow set with `--file` may sync only its name (unverified).
