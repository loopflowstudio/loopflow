. /tmp/lf-qa-978/env.sh $1
cd $ROOT
nohup "$BIN" server run < /dev/null > $ROOT/server.out 2> $ROOT/server.err &
echo $! > $ROOT/server.pid
sleep 3
"$BIN" server status < /dev/null > $ROOT/status.json; cat $ROOT/status.json | head -c 600; echo
