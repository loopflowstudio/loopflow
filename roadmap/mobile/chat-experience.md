---
status: todo
phase: 3
---

# Chat Experience

LLM-powered conversation about code. No tools, no execution—just discussion and planning.

## Current

No chat interface. Mobile can only trigger actions (Phase 2).

## Build

**Chat interface:**
- Conversation view on iOS/iPad
- Markdown rendering for code snippets
- Conversation history persistence

**LLM integration:**
- Claude API, OpenAI API, Gemini API (user's choice)
- API keys stored in Keychain
- Streaming responses

**Context assembly:**
- Current wave state (area, flow, step)
- Relevant files from area
- Recent diff
- PR description if applicable

**Suggestions:**
- Chat can suggest steps to run
- "Run `lf debug`" becomes tappable action
- Bridges to Phase 2 non-interactive execution

## Example conversations

"What's wrong with this PR?"
→ LLM reviews diff, explains issues

"How should I approach this bug?"
→ LLM suggests debugging strategy

"Review this diff for security issues"
→ LLM analyzes changes

"What does the WaveService protocol do?"
→ LLM explains codebase

## What's NOT included

- File editing (Phase 4)
- Command execution (Phase 4)
- Making commits (Phase 4)

Chat is for thinking. Execution happens via Phase 2 actions or at your laptop.

## Done when

User can have a conversation about their code from iPhone/iPad, with relevant context assembled automatically.
