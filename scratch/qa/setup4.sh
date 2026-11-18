# A repository with nothing under .lf in Git: the personal file is its only settings layer.
set -e
. /tmp/lf-qa-978/env.sh $1
rm -rf $ROOT/repo2 $ROOT/r2wt; mkdir -p $HOME $ROOT/repo2; cd $ROOT/repo2
git init -q -b main; git config user.email qa@example.com; git config user.name QA
echo hello > README.md; git add -A; git commit -qm init
L config set agent claude:opus
git worktree add -q -b r2 $ROOT/r2wt
(cd $ROOT/r2wt && L task create "Appearing checkout" --worktree $ROOT/r2wt --json > $ROOT/r2task.json)
git status --short; ls -a . ; cat lf.config.local.yaml 2>/dev/null; head -c 400 $ROOT/r2task.json
