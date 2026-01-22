// Service for loading voices from .lf/voices/*.md files.

import Foundation

public struct VoiceService: @unchecked Sendable {
    public init() {}
    public func loadVoices(from repoURL: URL) -> [Voice] {
        let voicesDir = repoURL.appendingPathComponent(".lf/voices")
        let fm = FileManager.default

        guard fm.fileExists(atPath: voicesDir.path) else {
            return []
        }

        guard let files = try? fm.contentsOfDirectory(at: voicesDir, includingPropertiesForKeys: nil) else {
            return []
        }

        return files
            .filter { $0.pathExtension == "md" }
            .compactMap { parseVoiceFile($0) }
            .sorted { $0.name < $1.name }
    }

    private func parseVoiceFile(_ url: URL) -> Voice? {
        guard let content = try? String(contentsOf: url, encoding: .utf8) else {
            return nil
        }

        let name = url.deletingPathExtension().lastPathComponent

        return Voice(
            id: name,
            name: name,
            content: content.trimmingCharacters(in: .whitespacesAndNewlines),
            path: url
        )
    }
}
