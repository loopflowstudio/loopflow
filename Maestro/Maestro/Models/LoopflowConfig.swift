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

/// External skill library configuration.
struct SkillSourceConfig: Codable {
    let name: String
    let prefix: String
    let path: String
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
    let chrome: Bool?
    let docs: Bool?
    let diff: Bool?
    let diffFiles: Bool?
    let paste: Bool?
    let voice: VoiceConfig?
    let summaries: [SummaryConfig]?
    let skillSources: [SkillSourceConfig]?
    let work: WorkConfig?

    enum CodingKeys: String, CodingKey {
        case agentModel = "agent_model"
        case diffFiles = "diff_files"
        case skillSources = "skill_sources"
        case interactive, terminal, ide, workspace, context, exclude, push, pr, yolo, chrome, docs, diff, paste, voice, summaries, work
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

/// Available models for the lf CLI.
struct AgentModel: Identifiable, Hashable {
    let id: String  // e.g., "claude:opus", "codex:o3"
    let backend: String
    let variant: String?

    var displayName: String {
        if let variant = variant {
            return "\(backend.capitalized) \(variant.capitalized)"
        }
        return backend.capitalized
    }

    var cliValue: String { id }

    init(_ value: String) {
        self.id = value
        let parts = value.split(separator: ":", maxSplits: 1)
        self.backend = String(parts[0])
        self.variant = parts.count > 1 ? String(parts[1]) : nil
    }

    static let claude = AgentModel("claude")
    static let claudeOpus = AgentModel("claude:opus")
    static let claudeSonnet = AgentModel("claude:sonnet")
    static let codex = AgentModel("codex")
    static let codexO3 = AgentModel("codex:o3")
    static let codexO4Mini = AgentModel("codex:o4-mini")
    static let gemini = AgentModel("gemini")

    /// Common models shown in the picker.
    static let commonModels: [AgentModel] = [
        .claude,
        .claudeOpus,
        .claudeSonnet,
        .codex,
        .codexO3,
        .codexO4Mini,
        .gemini
    ]
}
