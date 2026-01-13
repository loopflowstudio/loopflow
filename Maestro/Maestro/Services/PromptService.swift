// Service for loading prompts from .claude/commands/.

import Foundation

struct PromptService {
    func loadPrompts(from repoURL: URL, config: LoopflowConfig?) throws -> [PromptCard] {
        let commandsDir = repoURL.appendingPathComponent(".claude/commands")

        guard FileManager.default.fileExists(atPath: commandsDir.path()) else {
            return []
        }

        let contents = try FileManager.default.contentsOfDirectory(
            at: commandsDir,
            includingPropertiesForKeys: nil
        )

        return contents
            .filter { $0.pathExtension == "md" }
            .compactMap { url -> PromptCard? in
                guard let content = try? String(contentsOf: url, encoding: .utf8) else {
                    return nil
                }

                let name = url.deletingPathExtension().lastPathComponent
                let isInteractive = config?.isInteractive(name) ?? false

                return PromptCard(
                    name: name,
                    content: content,
                    defaultMode: isInteractive ? .interactive : .auto
                )
            }
            .sorted { $0.name < $1.name }
    }
}
