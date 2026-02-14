// Row view for an orphan worktree in the "On Disk" sidebar section.

import SwiftUI
import LoopflowCore

struct WorktreeRow: View {
    let worktree: WorktreeInfo
    let onUpgrade: () -> Void

    @State private var isHovering = false

    private let launcher = TerminalLauncher()

    private var displayName: String {
        worktree.branch ?? worktree.shortName ?? "detached"
    }

    var body: some View {
        HStack(spacing: Spacing.sm) {
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                Text(displayName)
                    .fontWeight(.medium)
                    .foregroundStyle(.white.opacity(worktree.merged ? 0.4 : 1.0))
                    .strikethrough(worktree.merged)
                    .lineLimit(1)

                Text(worktree.directoryName)
                    .font(Typography.caption())
                    .foregroundStyle(.white.opacity(0.4))
                    .lineLimit(1)
            }

            Spacer()

            if worktree.merged {
                Text("merged")
                    .font(Typography.caption(10))
                    .foregroundStyle(Color.statusNeutral)
            } else if isHovering {
                Button {
                    onUpgrade()
                } label: {
                    Text("Upgrade")
                        .font(Typography.caption(10))
                        .fontWeight(.medium)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(Color.white.opacity(0.15))
                        .foregroundStyle(.white.opacity(0.7))
                        .clipShape(Capsule())
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, Spacing.md)
        .padding(.vertical, Spacing.sm)
        .background(
            RoundedRectangle(cornerRadius: CornerRadius.md)
                .fill(isHovering ? Color.white.opacity(0.08) : Color.clear)
        )
        .contentShape(Rectangle())
        .onHover { hovering in
            isHovering = hovering
        }
        .opacity(worktree.merged ? 0.6 : 1.0)
        .contextMenu {
            Button {
                launcher.openInFinder(at: URL(fileURLWithPath: worktree.path))
            } label: {
                Label("Open in Finder", systemImage: "folder")
            }

            Button {
                try? launcher.launchTerminal(.warp, at: URL(fileURLWithPath: worktree.path))
            } label: {
                Label("Open in Warp", systemImage: "terminal")
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Worktree: \(displayName)")
    }
}
