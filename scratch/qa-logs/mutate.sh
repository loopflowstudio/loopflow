#!/bin/zsh
# usage: mutate.sh <name> <file> <sed-expr> <cargo nextest args...>
name=$1 file=$2 expr=$3; shift 3
cd /Users/jack/src/loopflow.loo-889-check-decisions-in
sed -i '' "$expr" "$file"
git diff --stat -- "$file" | head -1
U="$(env | grep -E '^(LF|REX)_' | cut -d= -f1 | sed 's/^/-u /')"
env ${=U} cargo nextest run -p loopflow --no-fail-fast "$@" < /dev/null > scratch/qa-logs/mut-$name.log 2>&1
echo "$name exit=$?"
git checkout -- "$file"
grep -E "^ +(PASS|FAIL)|Summary|error\[" scratch/qa-logs/mut-$name.log
