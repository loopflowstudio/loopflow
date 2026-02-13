# Open questions

1. Do `MemoryEditLog`, `ToolCallLog`, and `ContextSnapshot` need schema changes before persistence/API layers lock wire format?
2. If a turn fails before a final message, should lfd emit a synthetic final error message or only stream terminal failure events?
3. Should memory-edit persistence be committed per tool call or batched per turn with recovery semantics?
