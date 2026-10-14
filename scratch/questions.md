# Questions

- Interactive import relaunches the Session's skill (if any) with the
  continuation turn as its message, matching `open_waiting`'s retry. If the
  re-delivered skill instructions make the new agent restart work instead of
  continuing, launch import turns with `:` instead.
- The log lives in the logged conversation's capture directory rather than the
  new capture's (see the design's implementation notes).
