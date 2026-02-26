# Open questions

- Quote-reply capture is currently enabled only for assistant message bubbles. Should quote-reply also work on earlier user/system/error messages?
- Queue management in milestone 2 currently supports add + delete (no reorder/edit UI yet). Is reorder/edit required for this milestone or can it wait for a follow-up pass?
- The macOS selectable message renderer uses plain text for assistant messages when quote-reply is active, so markdown styling is not preserved in those bubbles. If markdown fidelity is required, we should add attributed markdown rendering in the native text view.
