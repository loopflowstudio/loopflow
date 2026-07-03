#if os(macOS)
import SwiftUI
import LoopflowCore

/// WaveChat's first surface: a scrollable transcript of a wave's turns, rendered
/// with the recovered `MessageRow`. This slice reconstructs the turns from the
/// wave's on-disk `GOAL.md` + `MEMORY.md` (see `WaveChatSource`) — a "latest
/// state" snapshot, not a live stream. A per-wave chat server replaces the source
/// later without touching this view.
struct WaveChatView: View {
    let repoPath: String
    let waveName: String

    @Environment(\.palette) private var palette
    @State private var turns: [SessionMessage] = []
    @State private var didLoad = false

    private static let timestampFormatter: RelativeDateTimeFormatter = {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter
    }()

    var body: some View {
        Group {
            if turns.isEmpty {
                emptyState
            } else {
                transcript
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(palette.background)
        .task(id: waveName) {
            guard !didLoad else { return }
            // Two small on-disk reads; cheap enough to do inline. A per-wave chat
            // server replaces this source with a live stream later.
            turns = WaveChatSource().loadTurns(repoPath: repoPath, waveName: waveName)
            didLoad = true
        }
    }

    private var transcript: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: Spacing.lg) {
                ForEach(turns) { turn in
                    MessageRow(message: turn, timestampLabel: timestampLabel(for: turn))
                }
            }
            .padding(.horizontal, Spacing.xl)
            .padding(.vertical, Spacing.lg)
            .frame(maxWidth: 720, alignment: .leading)
            .frame(maxWidth: .infinity, alignment: .center)
        }
        .accessibilityIdentifier("wave-chat-transcript")
    }

    private var emptyState: some View {
        VStack(spacing: Spacing.md) {
            Image(systemName: "bubble.left.and.bubble.right")
                .font(Typography.heroTitle(28))
                .foregroundStyle(palette.textSecondary.opacity(0.5))
            Text("No conversation yet")
                .font(Typography.sectionTitle())
                .foregroundStyle(palette.text)
            Text("This wave's goal and memory appear here as it runs.")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    private func timestampLabel(for turn: SessionMessage) -> String {
        Self.timestampFormatter.localizedString(for: turn.timestamp, relativeTo: Date())
    }
}

#endif
