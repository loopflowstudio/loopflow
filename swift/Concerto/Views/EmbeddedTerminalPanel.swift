// Embedded terminal panel using Ghostty.
// Shown instead of OutputPanel when the embeddedTerminal feature flag is enabled.

import SwiftUI

struct EmbeddedTerminalPanel: View {
    @Bindable var appState: AppState
    @State private var isExpanded = false
    @StateObject private var ghosttyManager = GhosttyManager.shared
    @Environment(\.colorScheme) private var colorScheme

    private var palette: LoopflowPalette {
        LoopflowPalette.make(for: colorScheme)
    }

    var body: some View {
        VStack(spacing: 0) {
            // Header bar
            if !appState.activeSessionIds.isEmpty || hasActiveWorktree {
                terminalHeader
            }

            // Terminal content
            if isExpanded, let worktree = effectiveWorktree {
                terminalContent(worktree: worktree)
            }
        }
    }

    private var hasActiveWorktree: Bool {
        appState.selectedWorktree != nil || !appState.activeSessionIds.isEmpty
    }

    private var effectiveWorktree: String? {
        appState.selectedWorktree?.path
    }

    private var terminalHeader: some View {
        HStack {
            Image(systemName: "terminal")
                .font(.caption)
                .foregroundStyle(.secondary)

            if !appState.activeSessionIds.isEmpty {
                Circle()
                    .fill(.green)
                    .frame(width: 8, height: 8)

                Text("\(appState.activeSessionIds.count) running")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } else {
                Circle()
                    .fill(.gray)
                    .frame(width: 8, height: 8)

                Text("idle")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer()

            // Status indicator for Ghostty
            switch ghosttyManager.state {
            case .uninitialized:
                Text("Terminal not initialized")
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
            case .initializing:
                ProgressView()
                    .scaleEffect(0.5)
            case .ready:
                EmptyView()
            case .failed(let error):
                Label("Error", systemImage: "exclamationmark.triangle")
                    .font(.caption2)
                    .foregroundStyle(.red)
                    .help(error)
            }

            // Expand/collapse toggle
            Button {
                withAnimation(.easeInOut(duration: 0.2)) {
                    isExpanded.toggle()
                }
            } label: {
                Image(systemName: isExpanded ? "chevron.down" : "chevron.up")
                    .font(.caption)
            }
            .buttonStyle(.plain)
            .help(isExpanded ? "Collapse" : "Expand")
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(palette.surface)
    }

    @ViewBuilder
    private func terminalContent(worktree: String) -> some View {
        GhosttyTerminalView(
            workingDirectory: worktree,
            manager: ghosttyManager
        )
        .frame(height: 200)
        .background(Color.black)
    }
}

#Preview {
    let state = AppState()
    return EmbeddedTerminalPanel(appState: state)
        .frame(width: 600)
}
