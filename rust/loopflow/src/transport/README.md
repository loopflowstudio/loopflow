# Retain execution across terminal loss

```sh
lf session attach SESSION
lf session attach SESSION --observe
lf --machine build session attach SESSION
lf transport start --terminal -- /bin/bash -l
lf transport attach PROCESS
```

Ctrl+] detaches a viewer. Ctrl+C in an attached terminal reaches the running
program. Closing a terminal leaves execution, provider requests and capture
settlement with the detached owner. Observing takes no input claim; a controlling
attachment revokes the previous viewer's input token.

`lf resume` reconnects to a retained native terminal. A structured Run may require
an explicit transition to the provider's native UI. That transition preserves the
Session and saved conversation, but does not preserve its old in-flight turn or
resubmit its input. `session attach` never performs that transition.

An unavailable owner leaves delivery unknown. Reconnection does not launch another
provider or replay a Flow. Finished Runs retain results in Session history.
After connecting, attachment waits up to five seconds for acknowledgement. Timeout
or Ctrl+C while waiting closes only the viewer; execution remains unknown. Once
acknowledged, the attachment has no time limit.

Terminal snapshots retain a text grid and up to 2,000 scrollback rows. They restore
alternate-screen state, application keys, bracketed paste and mouse modes supported
by vt100. Images, hyperlinks, extended keyboard protocols and exact restoration of
both screen buffers are not supported. Headless observation retains a bounded output
tail and preserves stdout/stderr on delivery. Slow viewers disconnect; execution
continues draining.

`transport serve` owns a command's PTY or pipes and is normally launched by `start`.
It records an ordinary Process. Session rows name that Process; viewer tokens and
terminal bytes remain in memory. Native clients retain their original provider
channels, including pending request identities. Flow steps share their Flow's
terminal owner through explicit child handoff, retaining their own Process results.
