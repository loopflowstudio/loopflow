# Maestro Output Streaming

Complete the output streaming feature by adding live output display to Maestro.

## Status

Implemented. All changes complete:
- `LFDEventService.swift`: Extended to parse `session.*` and `output.line` events
- `AppState.swift`: Added `liveOutputBySession` and `activeSessionIds` state
- `OutputPanel.swift`: New view with collapsible live output display
- `ContentView.swift`: Integrated OutputPanel below PromptLauncher
- Xcode project updated to include new file

## Context

This branch added `output.line` events to lfd (`.design/output-streaming.md`):
- `server.py`: Handler that broadcasts to subscribers
- `collector.py`: Sends each formatted line via fire-and-forget
- `api.md`: Documented the method and event

What's missing: Maestro Swift UI to subscribe and display the streaming output.

## Design

### Event Model

Extend the event types to handle output events:

```swift
// In LFDEventService.swift
enum LFDEvent: Sendable {
    case worktree(WorktreeEvent)
    case session(SessionEvent)
    case output(OutputEvent)
}

struct OutputEvent: Sendable {
    let sessionId: String
    let text: String
    let timestamp: Date
}

struct SessionEvent: Sendable {
    let id: String
    let task: String?
    let status: String?
}
```

### AppState Extensions

Add output state to track live output per session:

```swift
// In AppState.swift
var liveOutputBySession: [String: [OutputLine]] = [:]
var activeSessionIds: Set<String> = []

struct OutputLine: Identifiable {
    let id = UUID()
    let text: String
    let timestamp: Date
}
```

### Event Subscription

Extend `startEventSubscription()` to handle `output.line` and `session.*` events:

```swift
func startEventSubscription() {
    eventService = LFDEventService()

    Task {
        try? await eventService?.subscribe(
            to: ["worktree.*", "session.*", "output.line"]
        ) { [weak self] event in
            Task { @MainActor in
                switch event {
                case .worktree:
                    await self?.refreshWorktrees()
                case .session(let sessionEvent):
                    self?.handleSessionEvent(sessionEvent)
                case .output(let outputEvent):
                    self?.handleOutputEvent(outputEvent)
                }
            }
        }
    }
}

func handleSessionEvent(_ event: SessionEvent) {
    if event.status == "running" {
        activeSessionIds.insert(event.id)
        liveOutputBySession[event.id] = []
    } else if event.status == "completed" || event.status == "error" {
        activeSessionIds.remove(event.id)
        // Keep output buffer for viewing, clear after timeout
    }
}

func handleOutputEvent(_ event: OutputEvent) {
    guard activeSessionIds.contains(event.sessionId) else { return }

    let line = OutputLine(text: event.text, timestamp: event.timestamp)
    liveOutputBySession[event.sessionId, default: []].append(line)

    // Cap buffer at 1000 lines to prevent memory bloat
    if liveOutputBySession[event.sessionId]?.count ?? 0 > 1000 {
        liveOutputBySession[event.sessionId]?.removeFirst()
    }
}
```

### Output Panel View

Add a collapsible output panel below the prompt launcher:

```swift
// New file: Views/OutputPanel.swift
struct OutputPanel: View {
    @Bindable var appState: AppState
    @State private var isExpanded = false
    @State private var selectedSessionId: String?

    var body: some View {
        VStack(spacing: 0) {
            // Header bar - always visible when there's activity
            if !appState.activeSessionIds.isEmpty {
                outputHeader
            }

            // Expandable output area
            if isExpanded, let sessionId = selectedSessionId ?? appState.activeSessionIds.first {
                outputContent(sessionId: sessionId)
            }
        }
    }

    private var outputHeader: some View {
        HStack {
            // Session picker if multiple active
            if appState.activeSessionIds.count > 1 {
                Picker("Session", selection: $selectedSessionId) {
                    ForEach(Array(appState.activeSessionIds), id: \.self) { id in
                        Text(id.prefix(8)).tag(id as String?)
                    }
                }
                .labelsHidden()
            }

            // Activity indicator
            Circle()
                .fill(.green)
                .frame(width: 8, height: 8)

            Text("\(appState.activeSessionIds.count) running")
                .font(.caption)
                .foregroundStyle(.secondary)

            Spacer()

            Button {
                isExpanded.toggle()
            } label: {
                Image(systemName: isExpanded ? "chevron.down" : "chevron.up")
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(.bar)
    }

    @ViewBuilder
    private func outputContent(sessionId: String) -> some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 2) {
                    ForEach(appState.liveOutputBySession[sessionId] ?? []) { line in
                        Text(line.text)
                            .font(.system(.caption, design: .monospaced))
                            .foregroundStyle(colorFor(line.text))
                            .textSelection(.enabled)
                            .id(line.id)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
            }
            .frame(height: 200)
            .background(Color(.textBackgroundColor))
            .onChange(of: appState.liveOutputBySession[sessionId]?.count) { _, _ in
                // Auto-scroll to bottom
                if let lastLine = appState.liveOutputBySession[sessionId]?.last {
                    proxy.scrollTo(lastLine.id, anchor: .bottom)
                }
            }
        }
    }

    private func colorFor(_ text: String) -> Color {
        if text.hasPrefix("→") { return .blue }
        if text.hasPrefix("✓") { return .green }
        if text.hasPrefix("✗") { return .red }
        return .primary
    }
}
```

### Integration with ContentView

Add the output panel to the detail view:

```swift
// In ContentView.swift
var body: some View {
    NavigationSplitView {
        WorktreeSidebar(appState: appState)
    } detail: {
        VStack(spacing: 0) {
            if appState.currentRepo != nil {
                PromptLauncher(appState: appState)
                OutputPanel(appState: appState)
            }
        }
    }
}
```

## Implementation Order

1. Extend `LFDEventService` to parse `session.*` and `output.line` events
2. Add output state to `AppState`
3. Create `OutputPanel.swift`
4. Integrate into `ContentView`

## What's NOT in scope

- **Output history**: Panel shows live output only. Historical output is in log files.
- **Full-text search**: Not needed for live monitoring. Use `grep` on log files.
- **Output filtering**: Local filter in Maestro is simpler than server-side filtering.

## Open Questions

None.
