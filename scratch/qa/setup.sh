#!/bin/zsh
set -e
source ${0:A:h}/env.sh
rm -rf $QA; mkdir -p $QA/bin $HOME $LF_HOME $CODEX_HOME $CLAUDE_CONFIG_DIR
cp /Users/jack/src/loopflow.loo-869-read-work-selection/target/debug/lf $LF
git init -q --bare $QA/origin.git
git init -q -b main $REPO; cd $REPO
git config user.email qa@example.com; git config user.name QA
mkdir -p .lf; printf 'default_agent: codex\nplanning: {provider: git, personal: true}\n' > .lf/config.yaml
echo qa > README.md; git add -A; git commit -qm init
git remote add origin $QA/origin.git; git push -q -u origin main
