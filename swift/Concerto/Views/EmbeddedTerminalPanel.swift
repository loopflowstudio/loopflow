#if os(macOS)
// Embedded terminal panel using Ghostty.

import SwiftUI
import LoopflowCore

struct EmbeddedTerminalPanel: View {
    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @State private var isExpanded = false
    @StateObject private var ghosttyManager = GhosttyManager.shared
    @State private var terminalHeight: CGFloat = 250
    @Environment(\.palette) private var palette
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

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
        !repoState.isRemoteTarget && repoState.selectedWave?.worktreePath != nil
    }

    private var effectiveWorktree: String? {
        guard !repoState.isRemoteTarget else { return nil }
        return repoState.selectedWave?.worktreePath ?? repoState.currentRepo?.path()
    }

    private var terminalHeader: some View {
        HStack {
            Image(systemName: "terminal")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            if let wave = repoState.selectedWave, wave.status == .running {
                Circle()
                    .fill(Color.statusSuccess)
                    .frame(width: 8, height: 8)

                Text("running")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            } else {
                Circle()
                    .fill(Color.statusNeutral)
                    .frame(width: 8, height: 8)

                Text("idle")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }

            Spacer()

            // Status indicator for Ghostty
            switch ghosttyManager.state {
            case .uninitialized:
                Text("Terminal not initialized")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
            case .initializing:
                ProgressView()
                    .scaleEffect(0.5)
            case .ready:
                EmptyView()
            case .failed(let error):
                Label("Error", systemImage: "exclamationmark.triangle")
                    .font(Typography.caption(10))
                    .foregroundStyle(Color.statusError)
                    .help(error)
            }

            // Expand/collapse toggle
            Button {
                withAnimation(DesignAnimation.standard(reduceMotion)) {
                    isExpanded.toggle()
                }
            } label: {
                Image(systemName: isExpanded ? "chevron.down" : "chevron.up")
                    .font(Typography.caption())
            }
            .buttonStyle(.plain)
            .help(isExpanded ? "Collapse" : "Expand")
            .accessibleButton("Toggle terminal panel", hint: isExpanded ? "Collapse" : "Expand")
            .minHitTarget()
        }
        .padding(.horizontal, Spacing.lg)
        .padding(.vertical, Spacing.sm)
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

#endif
