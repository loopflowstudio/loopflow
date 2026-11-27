# Questions

Open choices from `scratch/concept-review.html`. The plan takes the
recommended option for each, marked as made for review.

- **Lock build steps here, or separately?** Taken: pin all eight actions to
  commit SHAs in this PR. The alternative is landing the Dependabot removal
  alone and filing the lock as its own Task.
- **Does a person read a hand bump before it merges?** Taken: no new rule.
- **Should a bump refuse releases younger than some age?** Taken: not here;
  listed under the plan's deferred findings, unfiled.
- **Is the PAT behind `DEPENDABOT_MERGE_TOKEN` used anywhere else?** Not
  checkable from here. It decides whether the PAT is revoked or only the two
  stored copies are deleted. Doppler holds a copy the brief did not name.
- **Pin the Rust compiler too?** Taken: no; compiler and tools float.
- **What should say a version here has a known vulnerability?** Today nothing
  does. Taken: the docs say so; a person checks the dependency graph setting;
  the uncovered part is a deferred finding. The alternatives are "nothing
  needs to" and a scheduled check that files a Task.
- **Should GitHub refuse an unpinned action?** Taken: yes, by the repository
  setting, after merge.
- **Keep a pin to code published 14 hours earlier?** Taken: no; the
  `install-action` pin moved to v2.90.3 (18 days old, nextest 0.9.151).
