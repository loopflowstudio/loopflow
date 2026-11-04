#!/bin/sh
# Isolated configuration demo: no provider or installed settings are changed.
printf '%s\n' 'Demo fixture: using temporary personal and project settings.' >&2
for name in $(env | sed -n 's/^\(LF_[A-Za-z0-9_]*\)=.*/\1/p'); do
    unset "$name"
done
export LF_HOME=/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/lf-config-demo-nw8ak5gu/home
cd /var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/lf-config-demo-nw8ak5gu/project || exit 1
exec /Users/jack/src/loopflow.loo-714-recover-cleanly-from/target/debug/lf "$@"
