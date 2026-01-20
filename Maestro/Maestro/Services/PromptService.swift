// Service for loading prompts from .claude/commands/ and external skill sources.

import Foundation

struct PromptService {
    func loadPrompts(from repoURL: URL, config: LoopflowConfig?) throws -> [PromptCard] {
        var prompts: [PromptCard] = []

        // Load local prompts from .claude/commands/
        prompts.append(contentsOf: try loadLocalPrompts(from: repoURL, config: config))

        // Load external skills from configured sources
        if let skillSources = config?.skillSources {
            for source in skillSources {
                prompts.append(contentsOf: loadExternalSkills(source: source))
            }
        }

        // Auto-detect superpowers if not explicitly configured
        if config?.skillSources?.contains(where: { $0.prefix == "sp" }) != true {
            let defaultPaths = [
                FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".superpowers"),
                FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("superpowers"),
                repoURL.appendingPathComponent("superpowers")
            ]
            for path in defaultPaths {
                if FileManager.default.fileExists(atPath: path.path()) {
                    let autoSource = SkillSourceConfig(name: "superpowers", prefix: "sp", path: path.path())
                    prompts.append(contentsOf: loadExternalSkills(source: autoSource))
                    break
                }
            }
        }

        return prompts.sorted { $0.name < $1.name }
    }

    private func loadLocalPrompts(from repoURL: URL, config: LoopflowConfig?) throws -> [PromptCard] {
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
                    defaultMode: isInteractive ? .interactive : .auto,
                    source: .local
                )
            }
    }

    private func loadExternalSkills(source: SkillSourceConfig) -> [PromptCard] {
        let sourcePath = (source.path as NSString).expandingTildeInPath
        let skillsDir = URL(fileURLWithPath: sourcePath).appendingPathComponent("skills")

        guard FileManager.default.fileExists(atPath: skillsDir.path()) else {
            return []
        }

        guard let contents = try? FileManager.default.contentsOfDirectory(
            at: skillsDir,
            includingPropertiesForKeys: [.isDirectoryKey]
        ) else {
            return []
        }

        return contents.compactMap { skillDir -> PromptCard? in
            // Check if it's a directory
            guard (try? skillDir.resourceValues(forKeys: [.isDirectoryKey]))?.isDirectory == true else {
                return nil
            }

            // Look for SKILL.md
            let skillFile = skillDir.appendingPathComponent("SKILL.md")
            guard let content = try? String(contentsOf: skillFile, encoding: .utf8) else {
                return nil
            }

            // Normalize skill name
            let rawName = skillDir.lastPathComponent
            let normalizedName = normalizeSkillName(rawName)
            let prefixedName = "\(source.prefix):\(normalizedName)"

            return PromptCard(
                name: prefixedName,
                content: content,
                defaultMode: .auto,  // External skills default to auto
                source: .external(source.name)
            )
        }
    }

    /// Normalize directory name to skill name (e.g., "brainstorming" -> "brainstorm").
    private func normalizeSkillName(_ dirName: String) -> String {
        var name = dirName.lowercased()

        // Remove common suffixes
        if name.hasSuffix("ing") {
            name = String(name.dropLast(3))
        } else if name.hasSuffix("s") && !name.hasSuffix("ss") {
            name = String(name.dropLast(1))
        }

        // Common abbreviations
        if name == "test-driven-development" {
            return "tdd"
        }

        return name
    }
}
