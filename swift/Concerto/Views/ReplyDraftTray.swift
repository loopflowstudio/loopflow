import SwiftUI
import LoopflowCore

// MARK: - Reply Draft Tray

struct ReplyDraftTray: View {
    @Environment(\.palette) private var palette
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    @Bindable var queue: ReplyQueue
    @Binding var isExpanded: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header

            if isExpanded {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    ForEach(queue.entries) { entry in
                        ReplyDraftEntryRow(entry: entry) {
                            queue.remove(id: entry.id)
                        }
                    }
                }
                .padding(.horizontal, Spacing.md)
                .padding(.bottom, Spacing.sm)
            }
        }
    }

    private var header: some View {
        HStack(spacing: Spacing.sm) {
            Button {
                withAnimation(DesignAnimation.fast(reduceMotion)) {
                    isExpanded.toggle()
                }
            } label: {
                HStack(spacing: Spacing.xs) {
                    Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)

                    Text("\(queue.count) replies queued")
                        .font(Typography.caption())
                        .foregroundStyle(palette.text)
                }
            }
            .buttonStyle(.plain)
            .minHitTarget()
            .accessibilityLabel(isExpanded ? "Collapse queued replies" : "Expand queued replies")

            Spacer()
        }
        .padding(.horizontal, Spacing.md)
        .padding(.vertical, Spacing.xs)
    }
}

private struct ReplyDraftEntryRow: View {
    @Environment(\.palette) private var palette

    let entry: ReplyEntry
    let onDelete: () -> Void

    var body: some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                if let quoted = entry.quotedText {
                    Text("> \(truncate(quoted, limit: 80))")
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)
                        .lineLimit(2)
                }

                Text(truncate(entry.responseText, limit: 160))
                    .font(Typography.body())
                    .foregroundStyle(palette.text)
                    .lineLimit(3)
            }

            Spacer(minLength: 0)

            Button {
                onDelete()
            } label: {
                Image(systemName: "xmark.circle.fill")
                    .foregroundStyle(palette.textSecondary)
            }
            .buttonStyle(.plain)
            .minHitTarget()
            .accessibilityLabel("Delete queued reply")
        }
        .padding(.horizontal, Spacing.sm)
        .padding(.vertical, Spacing.xs)
        .background(
            RoundedRectangle(cornerRadius: CornerRadius.sm)
                .fill(palette.surfaceMuted)
        )
    }

    private func truncate(_ text: String, limit: Int) -> String {
        guard text.count > limit else { return text }
        let idx = text.index(text.startIndex, offsetBy: limit)
        return String(text[..<idx]) + "…"
    }
}

// MARK: - Reply Composer Popover

struct ReplyComposerPopover: View {
    @Environment(\.palette) private var palette

    let quoted: String
    @Binding var replyDraft: String

    let onSubmitText: () -> Void
    let onEmoji: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Reply to selection")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            Text("“\(quoted)”")
                .font(Typography.body())
                .foregroundStyle(palette.text)
                .lineLimit(4)

            TextField("Type a reply", text: $replyDraft, axis: .vertical)
                .textFieldStyle(.roundedBorder)
                .lineLimit(1...4)

            HStack(spacing: Spacing.xs) {
                ForEach(Array(zip(["👍", "👎", "✏️", "❓"], ["Thumbs up", "Thumbs down", "Edit", "Question"])), id: \.0) { emoji, label in
                    Button(emoji) {
                        onEmoji(emoji)
                    }
                    .buttonStyle(.bordered)
                    .minHitTarget()
                    .accessibilityLabel(label)
                }

                Spacer()

                Button("Queue") {
                    onSubmitText()
                }
                .buttonStyle(DarkButtonStyle())
                .disabled(replyDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }
        .padding(Spacing.md)
        .frame(width: 320)
    }
}

private struct ReplyDraftTrayPreviewHarness: View {
    @State private var queue = ReplyQueue.demoQueueTriple()
    @State private var isExpanded = true

    var body: some View {
        ReplyDraftTray(
            queue: queue,
            isExpanded: $isExpanded
        )
        .padding()
        .frame(width: 640)
    }
}

private struct ReplyComposerPopoverPreviewHarness: View {
    @State private var draft = "This should be a partial index on active rows only."

    var body: some View {
        ReplyComposerPopover(
            quoted: "we should add an index on created_at",
            replyDraft: $draft,
            onSubmitText: {},
            onEmoji: { _ in }
        )
    }
}

#Preview("Draft Tray") {
    ReplyDraftTrayPreviewHarness()
}

#Preview("Composer Popover") {
    ReplyComposerPopoverPreviewHarness()
        .padding()
}
