# Shared engine versus multiple execution paths

Research for Jack Heart · November 3, 2026. Historical evidence, not new direction.

Current FFI already shares Rust reads, subscriptions and writes. Sharing must include routing/defaults/admission/recovery; transport adapters are legitimate. Preserve one lifecycle owner and finish replacing each migrated path. Command receipts cannot solve the external-side-effect crash window solely in a gateway cache. Test parity and actual supervisor lifetime.

Complete source, citations and contrary evidence: `9e4587bb53fc4102e8682a3bba61e9776a6de5c2:scratch/research_notes/Lessons from earlier lfd attempts/shared-engine.md` (`git show` that reference). HTML studies and image assets remain in this checkout. Current implementation and remaining proof: [mobile-ios.md](../../mobile-ios.md).
