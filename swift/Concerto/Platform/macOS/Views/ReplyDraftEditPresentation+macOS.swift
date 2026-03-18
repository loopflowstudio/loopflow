#if os(macOS)
import SwiftUI

extension ReplyDraftEditPresentation {
    func body(content: Content) -> some View {
        content.popover(
            isPresented: $isPresented,
            attachmentAnchor: .point(.top),
            arrowEdge: .top
        ) {
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
    }
}
#endif
