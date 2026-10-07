# Carry a Task follow-up after syncing main

LOO-375's tested listing repair is committed as `9e352f22b`. On October 6,
`lf --task LOO-375 pr next parallel-git-reads` failed while replaying
`99464bee8..jack/make-lf-wt-list-fast-listing-timings` and restored the original
branch. The range includes six commits but only two first-parent entries:
a merge of main (`4dffe8c1a`) and the repair. Replaying the full range onto main
tries to apply already-present upstream changes and stops on an empty pick.

Replay the branch's first-parent entries in order. Use parent one for merges so
edits recorded by the merge are retained. Drop picks made redundant by the new
base, retain deliberately authored empty commits, and preserve the existing
atomic sequence abort on genuine conflict. Prove ordinary follow-up, merge-only
edits, and rollback after a later conflict in disposable repositories. Do not
modify the live Task's history or state outside supported Loopflow operations.

The installed CLI still needs a delivered repair before retrying LOO-375's
rotation. A branch binary must not operate on the installed Home. LOO-304's
independent read-latency contribution continues; PR #1465 already merged.

Checks: two focused regressions passed (1.47 s), covering redundant sync merges,
edits authored in a merge, retained intentional empty commits and complete abort
after a later conflict; formatting, diff checks and all-target Clippy passed.
The implementation uses Git's `--empty=drop` with first-parent revision selection
and parent one for merge replay; local Git 2.50.1 supports these options.
