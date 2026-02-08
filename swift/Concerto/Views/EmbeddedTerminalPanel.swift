// Embedded terminal panel using Ghostty.

import SwiftUI
import LoopflowCore

struct EmbeddedTerminalPanel: View {
    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @State private var isExpanded = false
    @StateObject private var ghosttyManager = GhosttyManager.shared
    @State private var terminalHeight: CGFloat = 250
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var palette: LoopflowPalette {
        LoopflowPalette.make(for: colorScheme)
    }

    var body: some View {
        VStack(spacing: 0) {
            // Header bar
            if hasActiveWorktree {
                terminalHeader
            }

            // Terminal content
            if isExpanded, let worktree = effectiveWorktree {
                terminalContent(worktree: worktree)
            }
        }
    }

    private var hasActiveWorktree: Bool {
        repoState.selectedWave?.worktreePath != nil
    }

    private var effectiveWorktree: String? {
        repoState.selectedWave?.worktreePath ?? repoState.currentRepo?.path()
    }

    private var terminalHeader: some View {
        HStack {
            Image(systemName: "terminal")
                .font(.caption)
                .foregroundStyle(.secondary)

            if let wave = repoState.selectedWave, wave.status == .running {
                Circle()
                    .fill(.green)
                    .frame(width: 8, height: 8)

                Text("running")
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
                withAnimation(DesignAnimation.standard(reduceMotion)) {
                    isExpanded.toggle()
                }
            } label: {
                Image(systemName: isExpanded ? "chevron.down" : "chevron.up")
                    .font(.caption)
            }
            .buttonStyle(.plain)
            .help(isExpanded ? "Collapse" : "Expand")
            .accessibleButton("Toggle terminal panel", hint: isExpanded ? "Collapse" : "Expand")
            .minHitTarget()
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
        .frame(height: terminalHeight)
        .background(Color.black)
        .overlay(alignment: .bottom) {
            // Resize handle
            Rectangle()
                .fill(Color.clear)
                .frame(height: 8)
                .contentShape(Rectangle())
                .gesture(
                    DragGesture()
                        .onChanged { value in
                            let newHeight = terminalHeight + value.translation.height
                            terminalHeight = max(100, min(500, newHeight))
                        }
                )
                .onHover { hovering in
                    if hovering {
                        NSCursor.resizeUpDown.push()
                    } else {
                        NSCursor.pop()
                    }
                }
        }
    }
}

#Preview {
    EmbeddedTerminalPanel()
        .environment(RepoState())
        .environment(OutputBuffer())
        .frame(width: 600)
}
