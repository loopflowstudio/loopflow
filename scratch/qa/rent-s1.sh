source ${0:A:h}/rent-env.sh
mk() { $LF task create --title "$1" --request-id "$2" --json < /dev/null 2>/dev/null | python3 -I -c 'import json,sys; d=json.loads(sys.stdin.read()); print(d.get("task",d).get("id"))'; }
step "Define an on-demand template"; run machine ondemand add qa --provider daytona --vcpus 1 --memory-gb 1 --max 2 --max-usd-per-hour 1
step "Rent"; run machine ondemand rent qa
step "Machines"; run machine list
T1=$(mk 'Recover a report' qa-r1); T2=$(mk 'Draft the summary' qa-r2); print "T1=$T1 T2=$T2"
print $T1 > $R/t1; print $T2 > $R/t2
step "Move T1 to the rented Machine"; run task move $T1 --machine qa-1
step "Move T2 to the rented Machine"; run task move $T2 --machine qa-1
step "T1 ownership"; run task show $T1
step "Calls so far"; cat $R/calls.log
