// Service for loading .lf/config.yaml.

import Foundation

struct ConfigLoader {
    enum ConfigError: LocalizedError {
        case notFound
        case parseError(String)

        var errorDescription: String? {
            switch self {
            case .notFound: return "Config file not found"
            case .parseError(let msg): return "Failed to parse config: \(msg)"
            }
        }
    }

    func load(from repoURL: URL) throws -> LoopflowConfig? {
        let configPath = repoURL.appendingPathComponent(".lf/config.yaml")

        guard FileManager.default.fileExists(atPath: configPath.path()) else {
            return nil
        }

        let contents = try String(contentsOf: configPath, encoding: .utf8)
        return parseYAML(contents)
    }

    func parseYAML(_ contents: String) -> LoopflowConfig? {
        var agentModel: String?
        var interactive: [String]?
        var terminal: String?
        var ide: String?
        var workspace: String?
        var context: [String]?
        var exclude: [String]?
        var push: Bool?
        var pr: Bool?
        var yolo: Bool?

        let lines = contents.components(separatedBy: .newlines)
        var currentList: String?
        var listItems: [String] = []

        for line in lines {
            let trimmed = line.trimmingCharacters(in: .whitespaces)

            // Skip comments and empty lines
            if trimmed.isEmpty || trimmed.hasPrefix("#") {
                continue
            }

            // Handle list items
            if trimmed.hasPrefix("- ") {
                if let key = currentList {
                    let value = String(trimmed.dropFirst(2)).trimmingCharacters(in: .whitespaces)
                        .trimmingCharacters(in: CharacterSet(charactersIn: "\"'"))
                    listItems.append(value)
                    switch key {
                    case "interactive": interactive = listItems
                    case "context": context = listItems
                    case "exclude": exclude = listItems
                    default: break
                    }
                }
                continue
            }

            // Handle key: value pairs
            if let colonIndex = trimmed.firstIndex(of: ":") {
                // Save any pending list
                currentList = nil
                listItems = []

                let key = String(trimmed[..<colonIndex]).trimmingCharacters(in: .whitespaces)
                let valueRaw = String(trimmed[trimmed.index(after: colonIndex)...])
                    .trimmingCharacters(in: .whitespaces)

                // Check if this starts a list (value is empty or just whitespace)
                if valueRaw.isEmpty {
                    currentList = key
                    listItems = []
                    continue
                }

                let value = valueRaw.trimmingCharacters(in: CharacterSet(charactersIn: "\"'"))

                switch key {
                case "agent_model": agentModel = value
                case "terminal": terminal = value
                case "ide": ide = value
                case "workspace": workspace = value
                case "push": push = value == "true"
                case "pr": pr = value == "true"
                case "yolo": yolo = value == "true"
                default: break
                }
            }
        }

        return LoopflowConfig(
            agentModel: agentModel,
            interactive: interactive,
            terminal: terminal,
            ide: ide,
            workspace: workspace,
            context: context,
            exclude: exclude,
            push: push,
            pr: pr,
            yolo: yolo
        )
    }
}
