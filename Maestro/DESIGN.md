# Maestro Design Principles

Design research for the Maestro app—a visual interface for conducting AI coding agents.

## What to Build

A design system and UX philosophy for Maestro that embodies "musically inspired UX for conducting agents"—keeping humans in flow while orchestrating AI.

---

## The Maestro Metaphor

From loopflowstudio: The "Maestro" is an engineer who seeks balance between **craft and throughput**. They reject the false dichotomy between speed and quality. The conductor metaphor implies:

- **Human in the loop, not out of it** — watch, interrupt, resume
- **Arranging harmony** — multiple agents working together toward a unified outcome
- **Intentionality** — the maestro shapes the music; agents play the notes
- **Flow state** — the tool disappears; the work remains

> "The Maestro wants craft AND throughput, not either/or."

---

## Core Design Principles

### 1. Immediate Connection (Bret Victor)

**"Creators need an immediate connection to what they create."**

Any delay in the feedback loop between thinking and seeing means ideas that will never exist. For Maestro:

- Show agent progress in real-time, not after completion
- Stream output as it happens
- Make all state visible—what's running, what's waiting, what changed
- Sub-second response for all UI interactions

> "If there is any delay in that feedback loop between thinking of something and seeing it and building on it, then there is this whole world of ideas which will never be."

### 2. Progressive Disclosure (Notion, Stripe)

**Simple at the surface, infinite depth available.**

Notion achieves "effortless hierarchy"—clean and minimal at its core, but infinitely composable. Stripe's docs optimize for the "happy path" while making depth accessible.

For Maestro:
- Default view shows only what's needed: task, status, branch
- Complexity reveals on demand: logs, diffs, context files
- Don't front-load configuration; let users discover features through use
- The 80% case should require zero configuration

### 3. Speed as Feature (Linear, Figma)

**Sub-100ms response times. 60fps animations. No loading spinners.**

Linear and Figma prove that performance is not a technical metric but a product feature. Speed enables flow state.

For Maestro:
- Optimistic UI updates—show results before server confirmation
- Keyboard-first navigation (see Cmd+K below)
- Prefetch likely next states
- Never block the UI on network requests

> "The faster you make that feedback loop, the more you can get into that flow state." — Dylan Field

### 4. Keyboard-First (Linear)

**The command palette is the control center.**

Linear's Cmd+K design: every action searchable, shortcuts discoverable, no mouse required for power users.

For Maestro:
- `Cmd+K` opens global command palette
- All actions have keyboard shortcuts
- Shortcuts displayed alongside actions in menus
- Key-to-verb mapping: `R` for Run, `S` for Stop, `D` for Diff

### 5. Opinionated Defaults (Linear, fast.ai)

**Design for someone, not everyone.**

Linear refuses to build Jira-level complexity. fast.ai embeds best practices so users don't have to configure them.

For Maestro:
- One good workflow, not infinite configuration
- Strong defaults that work immediately
- Don't ask users to make decisions they don't care about
- "Keep simple things simple and make complex things possible"

> "I don't think you can build the optimal tool for anything if it's very flexible or endlessly customizable." — Karri Saarinen

### 6. Graduated Autonomy (Cursor)

**Match the interaction surface to the scope of change.**

Cursor provides three tiers: Tab (local), Cmd+K (scoped), Agent (autonomous). Each has appropriate safeguards.

For Maestro:
- Quick actions: single-task launches, inline status
- Standard mode: task execution with streaming output
- Agent mode: multi-step pipelines with checkpoints
- Always provide a path back to manual control

### 7. Transparency Over Automation (Cursor, Andy Matuschak)

**Show plans before execution. Break work into auditable steps.**

Cursor's Plan Mode decouples reasoning from execution. Matuschak warns against tools that collect without compounding.

For Maestro:
- Show what the agent will do before it does it
- Display context being sent to the LLM
- Make costs visible (tokens, time, API calls)
- Every action should be reversible or at least recoverable

### 8. Design Should Disappear (Jony Ive)

**"A lot of what we are doing is getting design out of the way."**

The interface should be invisible. Users engage with their work, not with design decisions.

For Maestro:
- Minimize chrome; maximize content
- No decorative elements that don't aid comprehension
- The app should feel like "nothing"—just you and your agents
- If users notice the interface, it's probably wrong

> "Simplicity is not the absence of clutter, that's a consequence of simplicity. Simplicity is somehow essentially describing the purpose and place of an object."

### 9. Remove Barriers (fast.ai, Paper)

**Accessibility without patronizing.**

fast.ai removes gatekeeping while respecting intelligence. Paper by FiftyThree provided five brushes and nine colors—constraints that liberated creativity.

For Maestro:
- Get users to a working result immediately
- Don't require understanding to start using
- Provide thoughtful constraints that focus creative energy
- No credentials, no setup wizards, no "getting started" friction

> "Deep learning has, until now, been a very exclusive game. We're breaking it open."

### 10. Craft Signals Care (Patrick Collison, Jony Ive)

**Beauty in visible details implies care in invisible ones.**

Stripe treats documentation as product. Apple obsesses over details users will never consciously notice.

For Maestro:
- Pixel-perfect alignment
- Considered typography (the burgundy + serif of loopflowstudio)
- Animations that feel physical and natural
- Error states that are helpful, not hostile

> "If you care about the infrastructure being holistically good, indexing on the superficial characteristics that you can actually observe is not an irrational thing to do."

---

## Interaction Patterns

### The Command Palette (Cmd+K)

From Linear, Cursor, and Superhuman research:

- **Ubiquitous access**: Same shortcut from anywhere
- **Fuzzy search**: Find actions even with imprecise queries
- **Shortcuts displayed**: Learning mechanism built-in
- **Recent items**: Fast access to common actions
- **Bidirectional toggle**: Same key opens and closes

### Diff Review UX

From Cursor research—code review is the bottleneck:

- Show changes as diffs before applying
- Allow partial acceptance (some changes, not others)
- Smart highlighting of important sections
- Gray out boilerplate
- Word-by-word acceptance for fine control

### Context Management

From Cursor and Stripe:

- **Automatic context**: Don't demand explicit specification
- **@ references**: Surgical override when needed
- **Visual token budget**: Show what's included, what's truncated
- **Privacy controls**: `.cursorignore` equivalent for sensitive files

### Session Tracking

From loopflowstudio existing patterns:

- Status badges per worktree (design, implement, review, polish)
- Real-time updates via daemon socket
- History queryable per worktree
- Live output streaming

---

## Visual Design Direction

### From loopflowstudio Website

- **Burgundy accent** (#722f37) — evokes "guitars, wine, cellos"
- **Serif typography** (Instrument Serif) — editorial quality, intentionality
- **White backgrounds** — clarity, professionalism
- **Generous whitespace** — premium spacing (980px max-width)

### Influences to Study

| Tool | Pattern to Adopt |
|------|------------------|
| **Notion** | Minimalism, slash commands, block-based composition |
| **Figma** | Performance obsession, multiplayer presence, professional respect |
| **Linear** | Speed, polish, opinionated workflows, keyboard-first |
| **Stripe** | Three-column layout, copy-paste code, progressive disclosure |
| **Cursor** | Inline AI, diff review, graduated autonomy |

### Anti-Patterns to Avoid

- Fun/playful aesthetics ("I don't want my tools to be fun. I want them to be good." — Karri Saarinen)
- Air-traffic-control dashboards
- Configuration screens requiring training
- Loading spinners that block interaction
- Decorative illustrations that don't aid comprehension

---

## What "Conducting Agents" Implies About UX

The musical metaphor suggests specific design decisions:

### 1. Real-Time Feedback
A conductor sees and hears the orchestra in real-time. The Maestro app must stream agent output live—not after completion.

### 2. Gesture-Based Control
Conductors communicate through gesture: tempo, dynamics, cues. Consider:
- Keyboard shortcuts as "gestures"
- Drag-and-drop for workflow composition
- Quick taps to start/stop/pause

### 3. Score as Artifact
The score (prompt file) is the authoritative source. The conductor interprets it, but the score persists. In Maestro:
- Prompts are files, not ephemeral chat
- Design docs capture intent
- Everything is versioned

### 4. Sections Working Together
An orchestra has sections (strings, brass, woodwinds) that must harmonize. In Maestro:
- Multiple agents can run in parallel
- Status shows what each "section" is doing
- The conductor (user) coordinates the whole

### 5. Rehearsal vs. Performance
Conductors have different modes: rehearsal (exploratory, can stop) vs. performance (committed). In Maestro:
- Interactive mode = rehearsal (interrupt, redirect)
- Auto mode = performance (run to completion)

---

## Research Summaries

### Don Norman — Design of Everyday Things

**Key principles for productivity tools:**
- **Feedback**: Immediate, clear responses to all actions
- **Discoverability**: Command palettes, searchable actions, progressive disclosure
- **Constraints**: Type systems, validation, confirmation dialogs
- **Error Recovery**: Undo/redo, version history, autosave
- **Conceptual Models**: Consistent patterns, familiar metaphors

> "Do not blame people when they fail to use your products properly. Take people's difficulties as signifiers of where the product can be improved."

### Jony Ive

**On simplicity:**
- Real simplicity comes from structural clarity, not cosmetic minimalism
- "Solutions should feel inevitable"
- Care about details users can't articulate
- Form follows function, but function isn't enough—joy matters

> "What we make stands testament to who we are."

### Raphael Schaad / Paper by FiftyThree

**On creative tools:**
- "Yes, And..." philosophy—always moving forward, no Back buttons
- Zero visual distractions—perfectly clean canvas
- Constraints as liberation—5 brushes, 9 colors
- Design for activity ("Sketch", "Write") not simulation ("Pencil", "Pen")
- The Rewind gesture—fun to use, intuitive, discoverable

> "Creativity shouldn't require an entry fee."

### Bret Victor

**On immediate connection:**
- Eliminate the compile-run cycle
- Show results as code is typed
- Enable "time travel"—scrubbing through execution
- Multiple simultaneous representations
- Adapt tools to human cognition, not reverse

> "The programmer has to imagine the execution of the program and never sees the data."

### Andy Matuschak

**On tools for thought:**
- Most software falls far short of being "transformative"
- Powerful tools function as mediums, not utilities
- Memory is infrastructure, not trivia
- Spaced repetition = "programmable attention"
- Design for compounding returns over time

> "'Better note-taking' misses the point; what matters is 'better thinking.'"

### Jeremy Howard / fast.ai

**On accessible tools:**
- Get users to success immediately
- Design for the "uncool" user (modest resources, non-elite background)
- Layered APIs: high-level defaults, mid-level customization, low-level access
- Sensible defaults that incorporate best practices
- Remove barriers without patronizing

> "If you truly understand something, you can explain it in an accessible way."

### Figma

**On professional tools:**
- Performance is table stakes—build testing infrastructure early
- Remove friction before adding features (the "Blockers" team)
- Simplicity requires active defense
- Bridge design and development explicitly
- Taste is the moat in an AI world

> "Good enough has become mediocre. Design quality, taste, and craft will define the winners."

### Linear

**On refined productivity:**
- Sub-100ms response for all interactions
- Keyboard-first with comprehensive shortcuts
- Professional aesthetics, not playful
- Opinionated defaults over configuration
- Quality as the north star metric
- Small teams, high standards

> "Quality is our first principle. Every other metric and decision flows from that."

### Stripe

**On developer experience:**
- Three-column layout: navigation, content, code
- Personalized code examples (auto-populated API keys)
- Interactive elements embedded where learning happens
- Documentation as product, not afterthought
- Beauty signals care

> "If Stripe is a monstrously successful business, but what we make isn't beautiful... I'll be much less happy."

### Cursor

**On AI assistance:**
- Three-tier model: Tab (local), Cmd+K (scoped), Agent (autonomous)
- Show plans before execution
- Graduated autonomy with safeguards
- Context is automatic, override is surgical
- Speed is fun—sub-second responses

> "Programmers should stay in the driver's seat."

---

## Constraints

These would require rewriting if guessed wrong:

1. **Performance budget**: All interactions must complete in <100ms
2. **Keyboard-first**: Every action must be keyboard-accessible
3. **Real-time streaming**: Output must stream, not batch
4. **Single-window focus**: Maestro is the cockpit, not a dashboard
5. **Prompt-file authority**: UI reflects files, not replaces them

---

## Done When

1. Design principles documented and approved
2. Key interaction patterns specified with examples
3. Visual direction established with color, typography, spacing
4. Research synthesized into actionable guidelines

**Verification:**
```bash
cat .design/designprinciples.md | head -100
# Should show comprehensive design principles document
```

---

## Open Questions

Captured for future sessions:

1. How does multiplayer/collaboration work in Maestro? (Multiple humans conducting?)
2. Should there be a "rehearsal" vs "performance" mode distinction in UI?
3. How do we visualize token budget without overwhelming?
4. What's the right balance of streaming detail vs. summary?
5. How do we handle the Notion research (rate-limited, incomplete)?

---

## Sources

- Design of Everyday Things (Don Norman)
- Jony Ive interviews and talks
- Raphael Schaad portfolio and Paper by FiftyThree analysis
- Bret Victor: "Inventing on Principle", "Learnable Programming", worrydream.com
- Andy Matuschak: notes.andymatuschak.org, "How can we develop transformative tools for thought?"
- Jeremy Howard / fast.ai: course philosophy, fastai paper, nbdev
- Figma engineering blog, Dylan Field interviews
- Linear Method documentation, Karri Saarinen talks
- Stripe documentation, Patrick Collison interviews
- Cursor documentation, founder interviews
- loopflowstudio monorepo (Cadenza, website)
