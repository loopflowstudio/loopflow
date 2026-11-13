source ${0:A:h}/rent-env.sh
T1=$(cat $R/t1); T2=$(cat $R/t2)
: > $R/calls.log
step "Give the rented Machine back"; run machine end qa-1
step "Provider and SSH calls during end"; cat $R/calls.log
step "On-demand listing"; run machine ondemand list
for T in $T1 $T2; do
  step "Ownership of $T"
  $LF task show $T --json < /dev/null 2>&1 | python3 -I -c 'import json,sys; o=json.load(sys.stdin)["ownership"]; print("owner:", o["owner"]); [print(" ", e["kind"], e["machine_label"], e["person"]) for e in o["log"]]'
done
step "Claim T1 again (already ours)"; run task claim $T1
