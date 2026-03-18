#if os(iOS)
import SwiftUI

extension ReplyDraftEditPresentation {
    func body(content: Content) -> some View {
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
#endif
