# Repository portfolio — overlap research, October 11

Jack Heart requested default discovery of all main/non-linked-worktree repositories
under ~/src, a changeable folder, or a manually selected portfolio across folders
and subfolders. Jack suggested lf portfolio or lf repos with Desktop consuming the
same owner, and requested examination of existing Tasks before choosing new work.
October 11: Jack Heart added portfolio management and reuse/replacement of the
existing Portfolio surface to LOO-462’s implementation plan. The active contract
and next slice now live in `get-me-using-lf.md`; this file retains research only.
No separate Task was filed. Command spelling and discovery details in the plan
are labeled implementation choices, not additional decisions attributed to Jack.

## Existing owners and historical designs

- LOO-427 (including consolidated LOO-431): shared repository identity, one window
  per repository across Machines, and Machine-local repo_root/default ~/src lookup.
  Product memory leaves remembered repository names open. This is the closest
  current foundation, not a complete configurable portfolio contract.
- LOO-426: explicit Work opening; LOO-364: primary repo/Wave/Task conversations.
  These own opening/continuity, not which repositories belong in a portfolio.
- PR #401, commit 92accec2ff: original portfolio dashboard, add/remove repositories,
  persisted UserDefaults membership, and ~/src main-worktree scanning.
- PR #728, de97af68e8: Open Repo/menu/deep-link entry through PortfolioService.addRepo.
- PR #1131, f96e7d51e0: shared PortfolioDiscovery for the control-room and Wave UI.
- PR #478, 76e896f598: historical lf repos add and repository DAG/edge APIs.
  wave/cross-repo/05-concerto-ui.md at that commit explicitly used an lfd-owned
  portfolio. Its GitHub-remote identity, daemon and parent/child authority are
  retired models, not requirements to restore.

## Pre-implementation split

PortfolioDiscovery.repos unions hard-coded ~/src scanning, UserDefaults portfolio
entries, and the launch repository. RepoScanner examines immediate child directories
and normalizes linked worktrees to their main checkout. PortfolioService owns manual
membership in Swift. Rust config::repository_root reads Machine-local repo_root,
but the Swift scanner does not consume it. Therefore changing CLI lookup root does
not change Desktop discovery; removing a saved entry cannot exclude a repo that
is rediscovered automatically. Manual-only selection is not represented.

The October 11 implementation (`d43aa248d3`, compressed at `50e2cf44d1`) replaces
this split with lf-owned selection and discovery. PortfolioService retains the
shared reading; RepositoryPath retains explicit-opening validation. The historical
owners above explain that replacement, not remaining implementation work.

Evidence: lf wave list/status product, read-only saved Task briefs/comments including
historical records, source and Git history. No exact current Task for configurable
roots/manual portfolio was found in the inspected records; this is not an exhaustive
live Linear search. Repository-wide roadmap read remained pending during research.
