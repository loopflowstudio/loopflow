#!/bin/zsh
# Stop every terminal owner this private Machine launched, with its descendants.
tree() { for c in $(pgrep -P $1); do tree $c; done; echo $1; }
for o in $(ps -axo pid=,command= | awk '/lf session serve/ && /(9496bf|lf-qa-835|qa-pi)/ && !/awk/ {print $1}'); do
  P=$(tree $o); echo "owner $o: $(echo $P | tr '\n' ' ')"; kill $(echo $P) 2>/dev/null; sleep 0.5; kill -9 $(echo $P) 2>/dev/null
done
true
