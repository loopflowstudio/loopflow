import Foundation

/// Reads a wave's chat turns from the best on-disk source available today.
///
/// There is no per-wave chat server yet, so this backs WaveChat with the two-file
/// wave surface `lf wave` already maintains: `wave/<name>/GOAL.md` (the operator's
/// standing directive) becomes the opening turn, and `wave/<name>/MEMORY.md` (the
/// wave's working memory — what the latest passes learned and did) becomes the
/// wave's reply. It's a faithful "latest state" snapshot, not a live transcript.
///
// TODO(wavechat): back with codex harness + per-wave chat server. Once each
// `lf wave <name>` process hosts an in-process chat API, load real streamed
// ChatTurns instead of reconstructing two turns from GOAL.md + MEMORY.md.
public struct WaveChatSource: Sendable {
    public init() {}

    /// Load the wave's turns from `<repoPath>/wave/<waveName>/`. Returns an empty
    /// list when neither file exists — the caller renders an empty state.
    public func loadTurns(repoPath: String, waveName: String) -> [SessionMessage] {
        let waveDir = URL(fileURLWithPath: repoPath)
            .appendingPathComponent("wave", isDirectory: true)
            .appendingPathComponent(waveName, isDirectory: true)

        var turns: [SessionMessage] = []

        if let goal = readTurn(waveDir.appendingPathComponent("GOAL.md"), stripFrontmatter: true) {
            turns.append(SessionMessage(role: .user, content: goal.body, timestamp: goal.modified))
        }
        if let memory = readTurn(waveDir.appendingPathComponent("MEMORY.md"), stripFrontmatter: false) {
            turns.append(SessionMessage(role: .assistant, content: memory.body, timestamp: memory.modified))
        }

        return turns
    }

    private func readTurn(_ url: URL, stripFrontmatter: Bool) -> (body: String, modified: Date)? {
        guard let raw = try? String(contentsOf: url, encoding: .utf8) else { return nil }
        let body = (stripFrontmatter ? Self.stripFrontmatter(raw) : raw)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard !body.isEmpty else { return nil }
        let modified = (try? url.resourceValues(forKeys: [.contentModificationDateKey]))?
            .contentModificationDate ?? Date()
        return (body, modified)
    }

    /// Drop a leading `---`-fenced YAML frontmatter block, if present.
    static func stripFrontmatter(_ text: String) -> String {
        guard text.hasPrefix("---") else { return text }
        let lines = text.components(separatedBy: "\n")
        guard lines.first?.trimmingCharacters(in: .whitespaces) == "---" else { return text }
        for index in 1..<lines.count where lines[index].trimmingCharacters(in: .whitespaces) == "---" {
            return lines[(index + 1)...].joined(separator: "\n")
        }
        return text
    }
}
