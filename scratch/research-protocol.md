# Application protocol and transport

Research for Jack Heart · November 3, 2026. Historical evidence, not new direction.

Typed Work reads/commands and binary terminal attachment have separate recovery semantics. HTTPS/WSS through host Tailscale Serve is a recommendation; SSH can carry the same application protocol. Phone↔service, service↔owner and executable↔store are separate compatibility boundaries. LOO-593 owns serialization and executable fixtures, not an independent mobile SDK.

Complete source, citations and contrary evidence: `9e4587bb53fc4102e8682a3bba61e9776a6de5c2:scratch/research-protocol.md` (`git show` that reference). HTML studies and image assets remain in this checkout. Current implementation and remaining proof: [mobile-ios.md](mobile-ios.md).
