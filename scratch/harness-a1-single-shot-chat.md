# A1 — Single-shot chat

The simplest possible chat: user sends a message, LLM responds, response is displayed. Memory exists and is included in the prompt, but only edited manually by the user. No agent loop, no tools.

## What we'll learn

What the chat UX feels like. How memory should be displayed and edited.

## Checkpoint

A user can send a message, see a response, read/edit memory, and have that memory included in the next prompt.

## Verification

- Send a message, get a response, edit a memory block, send another message, confirm the memory is in the prompt
- Feel: does the memory feel useful? Is it clear what the LLM "knows"? Would you actually edit it?

## Context

From the harness roadmap:

- **Memory** is long-term knowledge that persists across invocations. Owned and displayed by the chat system. Provided to the harness as input.
- **The chat system** is the user-facing product. Receives events from the harness. Displays conversation, lets users read/edit memory.
- This is Track A, the chat system track. A1 is pre-harness — direct LLM calls, no tool dispatch.
- A2 (next) will replace the direct LLM call with the harness, so A1's architecture should make that swap easy.
