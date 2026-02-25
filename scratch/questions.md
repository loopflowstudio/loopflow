# Open questions

- AgentAPI hardening design has two wave-integration variants: (a) unregister + explicitly advance waiting wave step on successful session end, and (b) unregister only and rely on scheduler tick. This implementation chose (b). Confirm whether automatic waiting-step advancement should be added in a follow-up.
