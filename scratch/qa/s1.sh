source ${0:A:h}/walk.sh
export LF=/tmp/lf-qa-867-bin
step "Create a Task"
out=$($LF task create --title 'Recover a report' --request-id qa-1 --json < /dev/null 2>&1); print -r -- "$out" | tail -c 600
T=$(print -r -- "$out" | python3 -I -c 'import json,sys; d=json.loads(sys.stdin.read()); print(d.get("task",d).get("id"))')
print "TASK=$T"
step "Ownership after create"
$LF task show $T --json < /dev/null 2>&1 | python3 -I -c 'import json,sys; d=json.load(sys.stdin); o=d.get("ownership") or d.get("task",{}).get("ownership"); print(json.dumps(o,indent=1)[:1500]) if o else print(list(d.keys()))'
step "Unassign"; run task unassign $T
step "Claim"; run task claim $T
step "Claim again"; run task claim $T
step "Location"; run task location $T
print $T > $ROOT/task1
