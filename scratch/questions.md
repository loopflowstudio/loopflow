# Open Questions

- Portfolio wave-selection uses a notification retry (immediate + 400ms) to handle newly opened repo windows. If this proves flaky, we may want a stronger handshake (window-ready ack or explicit routing through app state).
- Portfolio cards currently use the active connection snapshot at creation time. If users change connection settings while the portfolio is open, cards are not rebuilt automatically yet.
