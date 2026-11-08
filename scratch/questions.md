# LOO-774 assumptions

For the docs, `retired` means lifetime-policy shutdown without a replacement:
stop automatic reattachment and leave deliberate resume available. The Task
brief names this reason without defining its client action. This reversible
choice avoids reconnecting forever or restarting a provider automatically.

An abrupt disconnect cannot guarantee delivery of a final frame, so bare EOF
keeps the reason unknown. No open product question blocks writing the docs.
