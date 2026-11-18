set -e
. /tmp/lf-qa-978/env.sh $1
rm -rf $ROOT; mkdir -p $HOME $ROOT/repo; cd $ROOT/repo
git init -q -b main; git config user.email qa@example.com; git config user.name QA
mkdir -p .lf/skills/tidy
cat > .lf/config.yaml <<'Y'
default_agent: codex
graph: code
skills:
  tidy:
    agent: claude:haiku
Y
printf -- '---\ndescription: Tidy the fixture.\n---\nTidy up.\n' > .lf/skills/tidy/SKILL.md
echo hello > README.md
git add -A; git commit -qm init
L wave create alpha --objective 'Keep the fixture tidy' > /dev/null
cat >> .lf/wave/alpha/config.yaml <<'Y'
agent: claude:sonnet
skills:
  implement:
    agent: codex
Y
git add -A; git commit -qm 'wave settings'
L config set agent claude:opus > /dev/null || true
for i in $(seq 1 24); do L task create "Fixture task $i" --wave alpha --json > /dev/null; done
for i in 1 2 3 4 5 6; do
  git worktree add -q -b wt$i $ROOT/wt$i
  (cd $ROOT/wt$i && L task create "Checked out task $i" --wave alpha --worktree $ROOT/wt$i --json > /dev/null)
done
L task list | tail -5
