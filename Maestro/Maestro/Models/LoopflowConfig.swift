// LoopflowConfig model for .lf/config.yaml.

import Foundation

/// Voice config that handles both single string and array formats in YAML.
enum VoiceConfig: Codable {
    case single(String)
    case multiple([String])

    var names: [String] {
        switch self {
        case .single(let name): return [name]
        case .multiple(let names): return names
        }
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let single = try? container.decode(String.self) {
            self = .single(single)
        } else if let multiple = try? container.decode([String].self) {
            self = .multiple(multiple)
        } else {
            throw DecodingError.typeMismatch(
                VoiceConfig.self,
                DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Expected string or array of strings")
            )
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .single(let name):
            try container.encode(name)
        case .multiple(let names):
            try container.encode(names)
        }
    }
}

/// Per-path summary configuration.
struct SummaryConfig: Codable {
    let path: String
    let tokens: Int
    let model: String?
}

struct AsanaConfig: Codable {
    let projectId: String

    enum CodingKeys: String, CodingKey {
        case projectId = "project_id"
    }
}

struct WorkConfig: Codable {
    let backend: String?
    let asana: AsanaConfig?
    let autoRebase: Bool?
    let autoLand: Bool?

    enum CodingKeys: String, CodingKey {
        case backend, asana
        case autoRebase = "auto_rebase"
        case autoLand = "auto_land"
    }

    var isAsana: Bool {
        backend?.lowercased() == "asana"
    }
}

struct LoopflowConfig: Codable {
    let agentModel: String?
    let interactive: [String]?
    let terminal: String?
    let ide: String?
    let workspace: String?
    let context: [String]?
    let exclude: [String]?
    let push: Bool?
    let pr: Bool?
    let yolo: Bool?
    let docs: Bool?
    let diff: Bool?
    let diffFiles: Bool?
    let paste: Bool?
    let voice: VoiceConfig?
    let summaries: [SummaryConfig]?
    let work: WorkConfig?

    enum CodingKeys: String, CodingKey {
        case agentModel = "agent_model"
        case diffFiles = "diff_files"
        case interactive, terminal, ide, workspace, context, exclude, push, pr, yolo, docs, diff, paste, voice, summaries, work
    }

    /// Voice config can be a single string or array of strings in YAML.
    var voiceNames: [String] {
        voice?.names ?? []
    }

    var terminalApp: TerminalApp {
        switch terminal?.lowercased() {
        case "terminal": return .terminal
        case "iterm": return .iterm
        case "kitty": return .kitty
        default: return .warp
        }
    }

    var ideApp: IDEApp {
        switch ide?.lowercased() {
        case "vscode": return .vscode
        case "zed": return .zed
        default: return .cursor
        }
    }

    func isInteractive(_ promptName: String) -> Bool {
        interactive?.contains(promptName) ?? false
    }

    var hasSummaries: Bool {
        !(summaries?.isEmpty ?? true)
    }
}

enum TerminalApp: String, CaseIterable {
    case warp
    case iterm
    case terminal
    case kitty

    var displayName: String {
        switch self {
        case .warp: return "Warp"
        case .iterm: return "iTerm"
        case .terminal: return "Terminal"
        case .kitty: return "Kitty"
        }
    }
}

enum IDEApp: String, CaseIterable {
    case cursor
    case vscode
    case zed

    var displayName: String {
        switch self {
        case .cursor: return "Cursor"
        case .vscode: return "VS Code"
        case .zed: return "Zed"
        }
    }
}
