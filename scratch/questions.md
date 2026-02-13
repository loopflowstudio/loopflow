# Open questions

- `MemoryEditLog`, `ToolCallLog`, and `ContextSnapshot` were referenced but not fully specified in the first-commit contract doc. I implemented the minimal shapes below to keep the contract moving:
  - `MemoryEditLog { op, block, detail }`
  - `ToolCallLog { tool, args, result_summary }`
  - `ContextSnapshot { memory_tokens, history_tokens, total_tokens }`
  If these should match a different wire schema, update these structs before persistence/API integration.
