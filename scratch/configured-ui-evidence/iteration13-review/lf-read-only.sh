#!/bin/sh
case "$1" in
  pm)
    printf '%s\n' 'Review transport rejected this update; no provider write was attempted.' >&2
    exit 1
    ;;
  ls|roadmap|activity|ps|status|doctor)
    exec /Users/jack/src/loopflow.main-view-task/target/debug/lf "$@"
    ;;
  session)
    if [ "$2" = list ]; then
      exec /Users/jack/src/loopflow.main-view-task/target/debug/lf "$@"
    fi
    ;;
esac
printf '%s\n' 'Review transport permits reads only.' >&2
exit 1
