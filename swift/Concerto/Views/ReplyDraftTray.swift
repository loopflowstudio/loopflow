import SwiftUI
import LoopflowCore

// MARK: - Reply Draft Tray

struct ReplyDraftTray: View {
    @Environment(\.palette) private var palette
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
#if os(iOS)
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass
#endif

    @Bindable var queue: ReplyQueue
    @Binding var isExpanded: Bool

    @State private var editingEntryID: UUID?
    @State private var editingReplyDraft = ""

    var body: some View {
        trayContent
            .modifier(ReplyDraftEditPresentation(
                editingEntry: editingEntry,
                editingReplyDraft: $editingReplyDraft,
                isPresented: editComposerIsPresented,
                isCompact: isCompactEditPresentation,
                onSubmitText: saveEdit,
                onEmoji: saveEmojiEdit
            ))
    }

    private var trayContent: some View {
        VStack(alignment: .leading, spacing: 0) {
            header

            if isExpanded {
                List {
                    ForEach(queue.entries) { entry in
                        ReplyDraftEntryRow(
                            entry: entry,
                            onEdit: entry.isEditable ? { beginEditing(entry) } : nil,
                            onDelete: { delete(entry) }
                        )
                        .listRowInsets(EdgeInsets(
                            top: Spacing.xs,
                            leading: Spacing.md,
                            bottom: Spacing.xs,
                            trailing: Spacing.md
                        ))
                        .listRowSeparator(.hidden)
                        .listRowBackground(Color.clear)
                    }
                    .onMove(perform: queue.move)
                    .onDelete(perform: deleteEntries)
                }
                .listStyle(.plain)
                .scrollContentBackground(.hidden)
                .background(Color.clear)
                .scrollDisabled(queue.count <= 4)
                .frame(height: listHeight)
                .padding(.bottom, Spacing.xs)
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

    private var editingEntry: ReplyEntry? {
        guard let editingEntryID else { return nil }
        return queue.entries.first { $0.id == editingEntryID }
    }

    private var editComposerIsPresented: Binding<Bool> {
        Binding(
            get: { editingEntry != nil },
            set: { if !$0 { closeEditor() } }
        )
    }

    private var listHeight: CGFloat {
        CGFloat(min(max(queue.count, 1), 4)) * 84
    }

    private var isCompactEditPresentation: Bool {
#if os(iOS)
        horizontalSizeClass == .compact
#else
        false
#endif
    }

    private func beginEditing(_ entry: ReplyEntry) {
        editingEntryID = entry.id
        editingReplyDraft = entry.responseText
    }

    private func saveEdit() {
        guard let entry = editingEntry else { return }

        switch entry {
        case .quoteReply(_, let quoted, _):
            queue.update(id: entry.id, newEntry: .quoteReply(quoted: quoted, reply: editingReplyDraft))
            closeEditor()
        case .freeText:
            queue.update(id: entry.id, newEntry: .freeText(text: editingReplyDraft))
            closeEditor()
        case .emojiReact:
            break
        }
    }

    private func saveEmojiEdit(_ emoji: String) {
        guard let entry = editingEntry else { return }
        guard case .quoteReply(_, let quoted, _) = entry else { return }

        queue.update(id: entry.id, newEntry: .emojiReact(quoted: quoted, emoji: emoji))
        closeEditor()
    }

    private func delete(_ entry: ReplyEntry) {
        if editingEntryID == entry.id {
            closeEditor()
        }
        queue.remove(id: entry.id)
    }

    private func deleteEntries(at offsets: IndexSet) {
        let idsToDelete = offsets.map { queue.entries[$0].id }
        if let editingEntryID, idsToDelete.contains(editingEntryID) {
            closeEditor()
        }
        queue.remove(atOffsets: offsets)
    }

    private func closeEditor() {
        editingEntryID = nil
        editingReplyDraft = ""
    }
}

private struct ReplyDraftEntryRow: View {
    @Environment(\.palette) private var palette

    let entry: ReplyEntry
    let onEdit: (() -> Void)?
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
        .contentShape(Rectangle())
        .onTapGesture {
            onEdit?()
        }
    }

    private func truncate(_ text: String, limit: Int) -> String {
        guard text.count > limit else { return text }
        let idx = text.index(text.startIndex, offsetBy: limit)
        return String(text[..<idx]) + "…"
    }
}

// MARK: - Reply Composer Content (shared across platforms)

struct ReplyComposerContent: View {
    @Environment(\.palette) private var palette

    let title: String
    let quoted: String?
    @Binding var replyDraft: String
    let submitLabel: String

    let onSubmitText: () -> Void
    let onEmoji: ((String) -> Void)?

    init(
        title: String = "Reply to selection",
        quoted: String?,
        replyDraft: Binding<String>,
        submitLabel: String = "Queue",
        onSubmitText: @escaping () -> Void,
        onEmoji: ((String) -> Void)? = nil
    ) {
        self.title = title
        self.quoted = quoted
        _replyDraft = replyDraft
        self.submitLabel = submitLabel
        self.onSubmitText = onSubmitText
        self.onEmoji = onEmoji
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text(title)
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            if let quoted {
                Text("“\(quoted)”")
                    .font(Typography.body())
                    .foregroundStyle(palette.text)
                    .lineLimit(4)
            }

            TextField(quoted == nil ? "Type a message" : "Type a reply", text: $replyDraft, axis: .vertical)
                .textFieldStyle(.roundedBorder)
                .lineLimit(1...4)

            HStack(spacing: Spacing.xs) {
                if let onEmoji, quoted != nil {
                    ForEach(reactionEmojis, id: \.emoji) { emoji, label in
                        Button(emoji) {
                            onEmoji(emoji)
                        }
                        .buttonStyle(.bordered)
                        .minHitTarget()
                        .accessibilityLabel(label)
                    }
                }

                Spacer()

                Button(submitLabel) {
                    onSubmitText()
                }
                .buttonStyle(DarkButtonStyle())
                .disabled(replyDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }
        .padding(Spacing.md)
    }
}

// MARK: - Platform Presentation Wrappers

struct ReplyComposerPopover: View {
    let title: String
    let quoted: String?
    @Binding var replyDraft: String
    let submitLabel: String

    let onSubmitText: () -> Void
    let onEmoji: ((String) -> Void)?

    init(
        title: String = "Reply to selection",
        quoted: String?,
        replyDraft: Binding<String>,
        submitLabel: String = "Queue",
        onSubmitText: @escaping () -> Void,
        onEmoji: ((String) -> Void)? = nil
    ) {
        self.title = title
        self.quoted = quoted
        _replyDraft = replyDraft
        self.submitLabel = submitLabel
        self.onSubmitText = onSubmitText
        self.onEmoji = onEmoji
    }

    var body: some View {
        ReplyComposerContent(
            title: title,
            quoted: quoted,
            replyDraft: $replyDraft,
            submitLabel: submitLabel,
            onSubmitText: onSubmitText,
            onEmoji: onEmoji
        )
        .frame(width: 320)
    }
}

private struct ReplyDraftEditPresentation: ViewModifier {
    let editingEntry: ReplyEntry?
    @Binding var editingReplyDraft: String
    @Binding var isPresented: Bool
    let isCompact: Bool
    let onSubmitText: () -> Void
    let onEmoji: (String) -> Void

    func body(content: Content) -> some View {
#if os(iOS)
        if isCompact {
            content.sheet(isPresented: $isPresented) {
                if let editingEntry {
                    editComposer(for: editingEntry)
                    .presentationDetents([.medium])
                }
            }
        } else {
            content.popover(isPresented: $isPresented, arrowEdge: .top) {
                if let editingEntry {
                    editComposer(for: editingEntry)
                    .frame(width: 320)
                }
            }
        }
#else
        content.popover(isPresented: $isPresented, attachmentAnchor: .point(.top), arrowEdge: .top) {
            if let editingEntry {
                ReplyComposerPopover(
                    title: "Edit queued reply",
                    quoted: editingEntry.quotedText,
                    replyDraft: $editingReplyDraft,
                    submitLabel: "Save",
                    onSubmitText: onSubmitText,
                    onEmoji: editingEntry.quotedText == nil ? nil : onEmoji
                )
            }
        }
#endif
    }

    private func editComposer(for entry: ReplyEntry) -> ReplyComposerContent {
        ReplyComposerContent(
            title: "Edit queued reply",
            quoted: entry.quotedText,
            replyDraft: $editingReplyDraft,
            submitLabel: "Save",
            onSubmitText: onSubmitText,
            onEmoji: entry.quotedText == nil ? nil : onEmoji
        )
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
