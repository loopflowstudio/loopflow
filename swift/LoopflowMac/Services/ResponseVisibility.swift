import Foundation
import Loopflow

/// Provider identity plus newly visible response text. A prompt echo or an old
/// preview cannot finish Send → response observed. Text stays in memory only.
struct ResponseVisibility {
    let previous: SessionPreview?
    let before: String
    let submittedAt: Int64

    func matches(_ preview: SessionPreview, visible: String) -> Bool {
        guard ["codex", "codex_transcript"].contains(preview.source), !preview.providerTurn.isEmpty,
              preview.providerTurn != previous?.providerTurn,
              previous == nil || preview.providerThread == previous?.providerThread,
              preview.observedAt >= submittedAt else { return false }
        let anchor = Self.words(preview.text).split(separator: " ").prefix(6).joined(separator: " ")
        guard anchor.count >= 3, !Self.words(before).contains(anchor) else { return false }
        return Self.words(visible).contains(anchor)
    }

    private static func words(_ text: String) -> String {
        text.split(whereSeparator: { !$0.isLetter && !$0.isNumber }).joined(separator: " ")
    }
}
