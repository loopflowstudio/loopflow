#!/bin/zsh
cd /Users/jack/src/loopflow.loo-889-check-decisions-in
M=scratch/qa-logs/mutate.sh
A=rust/loopflow/src/ops/task/adoption.rs
N=rust/loopflow/src/lf/navigation.rs
AF=(--lib -E 'test(adoption::tests::)')
DF=(--lib -E 'test(ssh::dispatch_tests::)')
$M otherrepo $A 's/map_err(task_error)? != main {/map_err(task_error)? != main \&\& false {/' $AF
$M diffcheckout $A 's/if session_root != root {/if false \&\& session_root != root {/' $AF
$M sharedwt $A 's/if worktree.is_some() {/if false \&\& worktree.is_some() {/' $AF
$M difftasks $A 's/if checkout_task != session_task {/if false \&\& checkout_task != session_task {/' $AF
$M fwdagent $N 's/matches!(name, "--machine" | "--forward-agent")/matches!(name, "--machine")/' $DF
$M sepvalue $N "153s/if takes_separate_value/if false \&\& takes_separate_value/" $DF
echo SWEEP DONE
