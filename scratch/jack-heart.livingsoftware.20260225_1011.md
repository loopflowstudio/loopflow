# Living Waves

Waves are the persistent, adaptive unit in loopflow. Agents are ephemeral — spawned, focused, killed. Waves remember, mutate, replicate, and configure the agents they spawn.

This design doc covers the architectural investment, foundational capabilities, and UX polish needed to make waves truly self-sustaining.

## Framing

Two peer product values in loopflow:

| Value | Metaphor | Product manifestation |
|-------|----------|----------------------|
| **Musical** | Orchestration, chords, flow | Waves, forks, directions as voices, chords |
| **Living** | Self-sustaining, evolving | Wave memory, skill injection, surface-adaptive agents |

The "living" value bridges people from writing single PRs to writing evolving waves. A living wave:
- **Remembers** what worked and what didn't (wave memory)
- **Configures** every agent it spawns with the right tools (skill injection)
- **Adapts** its presentation to the surface (mobile buttons, TUI commands, headless prompts)
- **Replicates** via split-wave, fork flows, and chords

The unit of "living" is the wave, not the agent. Agents don't need memory because the wave provides curated context. Context management IS the memory system — but curated by the wave, not accumulated by the agent. An agent with memory is a chatbot. A wave with memory is a living process.

## Layer 1: Harness environment setup

### Problem

When a wave spawns an agent, the agent doesn't know about loopflow's capabilities. Built-in steps aren't available as slash commands. The agent can't read or write wave memory. Surface-specific affordances (mobile buttons, TUI commands) aren't configured.

### Design

`SessionHarness::start()` (becoming `Harness::start()` on agentapi branch) becomes the environment setup boundary. When a wave spawns an agent, `start()` materializes everything the agent needs based on:

1. **Wave context** — which wave, which step, which directions
2. **Available skills** — builtins + repo-local + global + external (superpowers, rams, etc.)
3. **Surface capabilities** — what this harness/agent can render

#### Skill injection

For Claude Code sessions: write all available steps as `.claude/commands/*.md` files in the cwd before the first turn. Don't overwrite existing files (user customizations win).

```
.claude/commands/
  design.md        ← built-in step, written by harness
  implement.md     ← built-in step, written by harness
  my-custom.md     ← user-defined, NOT overwritten
```

For other agents (Codex, Gemini, OpenCode): include skill descriptions in the system prompt as available actions. Agent-specific adaptation as those platforms add command support.

#### Implementation sketch

Add to `LaunchConfig` / `AgentConfig`:

```rust
pub struct AgentConfig {
    // ... existing fields ...
    /// Skills to inject into the agent's environment.
    pub skills: Vec<InjectableSkill>,
    /// Wave memory path for read/write.
    pub wave_memory_path: Option<PathBuf>,
    /// Surface-specific capability hints.
    pub surface: Surface,
}

pub struct InjectableSkill {
    pub name: String,
    pub content: String,
}

pub enum Surface {
    Headless,
    Session,
    Tui,
    Mobile,
}
```

`ClaudeHarness::start()` materializes skills to `.claude/commands/`. Other harnesses adapt as appropriate.

#### Skill collection

The engine already has all the pieces:
- `builtin_step_names()` + `get_builtin_step()` — all built-in steps
- `list_user_steps()` — repo-local `.lf/steps/` and `.claude/commands/`
- `list_global_steps()` — global `~/.lf/steps/` and `~/.claude/commands/`
- `discover_skill_sources()` — external skills (superpowers, rams)

A new `collect_injectable_skills(repo)` gathers everything, respecting precedence (repo-local > global > builtin), and returns `Vec<InjectableSkill>`.

#### Cleanup

Skills written by the harness should be cleaned up when the session ends. Options:
- Track which files were written and remove on `stop()`
- Use a marker comment in the file (e.g., `<!-- lf:injected -->`) and clean those on stop
- Write to a temporary `.claude/commands/.lf-injected/` subdirectory (if Claude Code supports subdirectories)

Decision: track and remove on `stop()`. Simple, no magic markers.

## Layer 2: Wave memory

### Problem

Waves have no persistent memory across runs. Each agent starts fresh. Observations from one run ("this test suite is flaky", "the auth module uses JWT not sessions", "the user prefers small PRs") are lost.

### Design

Each wave gets a memory directory:

```
wave/<name>/memory/
  architecture.md    ← what the codebase looks like
  patterns.md        ← what works, what doesn't
  preferences.md     ← user/project preferences observed
```

#### How agents interact with memory

**Read**: At prompt assembly time, wave memory files are included in the context budget alongside other docs (area docs, repo docs, diff). They're just another `DocumentSource` — `DocumentSource::WaveMemory`.

**Write**: The system prompt tells agents about the memory path and when to write:

```
You are working in wave "engbot". Observations that should persist across
future runs go in wave/engbot/memory/. Read existing memory at the start.
Update when you learn something durable about the codebase, user preferences,
or patterns. Don't write session-specific notes — only things future agents
in this wave should know.
```

No special tool needed. The agent has file access. The system prompt is the injection.

#### Memory lifecycle

- **Creation**: First agent to observe something durable writes it
- **Growth**: Subsequent agents add to existing topic files or create new ones
- **Consolidation**: The `consolidate` step (already exists) can include memory cleanup — merge duplicates, remove stale observations
- **Scope**: Memory is per-wave. Split-wave could optionally copy parent memory to children.

#### Context budget

Wave memory competes for tokens with other context sources. `trim_context_with_breakdown()` already handles this — wave memory gets a budget allocation alongside step, direction, diff, docs, area.

Priority ordering (when budget is tight):
1. Step content (the task)
2. Direction (the perspective)
3. Diff (what's changed)
4. Wave memory (what the wave knows)
5. Area docs
6. Repo docs

### What NOT to do

Don't build an agent memory system. Don't accumulate conversation history across runs. Don't give agents persistent identity. The wave provides context; the agent is stateless. This distinction is what keeps things clean.

## Layer 3: Surface-adaptive rendering

### Problem

The same wave step looks different on different surfaces: terminal, desktop app, mobile. Today the harness doesn't adapt — it sends the same system prompt everywhere.

### Design

The harness injects surface-specific capabilities into the agent's prompt:

#### Headless (auto mode)
- Wave memory path (read/write)
- Full skill descriptions in system prompt
- No interactive affordances

#### Session (daemon-managed)
- Wave memory path (read/write)
- Skills as `.claude/commands/` files
- Standard interactive session

#### TUI (`lf design`, human-driven)
- Maybe wave memory (read-only? or skip entirely — human is driving)
- Skills already available via `lf` CLI
- No injection needed — the human knows the commands

#### Mobile (Concerto iOS)
- Wave memory path (read-only — don't let phone agents write memory?)
- Action suggestions: agent emits structured suggestions, Concerto renders as buttons
- System prompt addition:

```
You are running on a mobile surface. You can suggest actions the user can
tap to execute. Emit suggestions as:

<suggestion label="Run tests" action="lf gate" />
<suggestion label="Focus on auth" action="set-area src/auth/" />
```

Concerto parses these from the agent's output and renders tappable buttons. The model is already structured output — we're just telling it what affordances exist.

## Roadmap

### Phase 0: Establish "living codebase" as a product pillar

Before building anything, establish the concept so it shapes everything downstream — not just this wave but how loopflow talks about itself, how directions guide work, and how the README frames the product.

- [ ] Write a `living` direction (values file alongside `craft.md`, `flow.md`, `scale.md`)
- [ ] Update README framing: loopflow helps you build living codebases, not just ship PRs
- [ ] Add "living" language to LOOPFLOW.md (the bundled system doc agents see)
- [ ] Review existing steps/directions through the living-codebase lens — do they reinforce or contradict?
- [ ] Frame the wave plan in `wave/` so future waves orient around this pillar

The direction file is the seed. Once it exists, every `lf implement -d living`, `lf review -d living`, `lf research -d living` evaluates work through this lens. The product value propagates through the system it describes.

### Phase 1: Skill injection (smallest thing that proves the pattern)

- [ ] Add `collect_injectable_skills(repo)` to engine
- [ ] Add skill injection to `ClaudeHarness::start()` — write `.claude/commands/` files
- [ ] Clean up injected files on `stop()`
- [ ] Add skill list to system prompt for non-Claude harnesses
- [ ] Test: session agent can `/design`, `/review`, etc.

### Phase 2: Wave memory

- [ ] Create `wave/<name>/memory/` directory structure
- [ ] Add `DocumentSource::WaveMemory` to context gathering
- [ ] Add wave memory instructions to system prompt during prompt assembly
- [ ] Budget allocation in `trim_context_with_breakdown()`
- [ ] Test: agent writes observation, next agent in same wave reads it

### Phase 3: Surface-adaptive prompts

- [ ] Add `Surface` enum to `AgentConfig`
- [ ] Surface-specific prompt sections in `format_context_prompt()`
- [ ] Mobile: action suggestion parsing in Concerto
- [ ] Mobile: button rendering for suggestions
- [ ] Test: same wave step produces different prompt on different surfaces

### Phase 4: Memory lifecycle

- [ ] Memory consolidation in `consolidate` step
- [ ] Memory inheritance on `split-wave`
- [ ] Memory size limits / staleness detection
- [ ] Wave summary includes memory highlights

## Open questions

- Should TUI agents (`lf design`) get wave memory? The human is driving, but reading memory could still be useful context.
- Should mobile agents write to wave memory or just read? Risk of low-quality observations from quick mobile interactions.
- How should external skills (superpowers, rams) be injected? Same `.claude/commands/` mechanism, or separate?
- Should skill injection happen on every turn (in case cwd changes) or only on `start()`?
- Naming: "living waves", "adaptive waves", "persistent waves", or something else entirely?
