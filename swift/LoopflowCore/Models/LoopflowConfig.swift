// LoopflowConfig model for .lf/config.yaml.

import Foundation

/// Direction config that handles both single string and array formats in YAML.
public enum DirectionConfig: Sendable, Codable {
    case single(String)
    case multiple([String])

    public var names: [String] {
        switch self {
        case .single(let name): return [name]
        case .multiple(let names): return names
        }
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let single = try? container.decode(String.self) {
            self = .single(single)
        } else if let multiple = try? container.decode([String].self) {
            self = .multiple(multiple)
        } else {
            throw DecodingError.typeMismatch(
                DirectionConfig.self,
                DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Expected string or array of strings")
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
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
public struct SummaryConfig: Sendable, Codable {
    public let path: String
    public let tokens: Int
    public let model: String?
}

public struct AsanaConfig: Sendable, Codable {
    public let projectId: String

    enum CodingKeys: String, CodingKey {
        case projectId = "project_id"
    }
}

public struct WorkConfig: Sendable, Codable {
    public let backend: String?
    public let asana: AsanaConfig?
    public let autoRebase: Bool?
    public let autoLand: Bool?

    enum CodingKeys: String, CodingKey {
        case backend, asana
        case autoRebase = "auto_rebase"
        case autoLand = "auto_land"
    }

    public var isAsana: Bool {
        backend?.lowercased() == "asana"
    }
}

/// External skill library configuration.
public struct SkillSourceConfig: Sendable, Codable {
    public let name: String
    public let prefix: String
    public let path: String
}

public struct LoopflowConfig: Sendable, Codable {
    public let waveModel: String?
    public let interactive: [String]?
    public let terminal: String?
    public let ide: String?
    public let workspace: String?
    public let context: [String]?
    public let exclude: [String]?
    public let push: Bool?
    public let pr: Bool?
    public let yolo: Bool?
    public let chrome: Bool?
    public let docs: Bool?
    public let diff: Bool?
    public let diffFiles: Bool?
    public let paste: Bool?
    public let direction: DirectionConfig?
    public let summaries: [SummaryConfig]?
    public let skillSources: [SkillSourceConfig]?
    public let work: WorkConfig?

    enum CodingKeys: String, CodingKey {
        case waveModel = "wave_model"
        case diffFiles = "diff_files"
        case skillSources = "skill_sources"
        case interactive, terminal, ide, workspace, context, exclude, push, pr, yolo, chrome, docs, diff, paste, direction, summaries, work
    }

    /// Direction config can be a single string or array of strings in YAML.
    public var directionNames: [String] {
        direction?.names ?? []
    }

    public var terminalApp: TerminalApp {
        switch terminal?.lowercased() {
        case "terminal": return .terminal
        case "iterm": return .iterm
        case "kitty": return .kitty
        default: return .warp
        }
    }

    public var ideApp: IDEApp {
        switch ide?.lowercased() {
        case "vscode": return .vscode
        case "zed": return .zed
        default: return .cursor
        }
    }

    public func isInteractive(_ promptName: String) -> Bool {
        interactive?.contains(promptName) ?? false
    }

    public var hasSummaries: Bool {
        !(summaries?.isEmpty ?? true)
    }
}

public enum TerminalApp: String, Sendable, CaseIterable {
    case warp
    case iterm
    case terminal
    case kitty

    public var displayName: String {
        switch self {
        case .warp: return "Warp"
        case .iterm: return "iTerm"
        case .terminal: return "Terminal"
        case .kitty: return "Kitty"
        }
    }
}

public enum IDEApp: String, Sendable, CaseIterable {
    case cursor
    case vscode
    case zed

    public var displayName: String {
        switch self {
        case .cursor: return "Cursor"
        case .vscode: return "VS Code"
        case .zed: return "Zed"
        }
    }
}

/// Available models for the lf CLI.
public struct WaveModel: Sendable, Identifiable, Hashable {
    public let id: String  // e.g., "claude:opus", "codex:o3"
    public let backend: String
    public let variant: String?

    public var displayName: String {
        if let variant = variant {
            return "\(backend.capitalized) \(variant.capitalized)"
        }
        return backend.capitalized
    }

    public var cliValue: String { id }

    public init(_ value: String) {
        self.id = value
        let parts = value.split(separator: ":", maxSplits: 1)
        self.backend = String(parts[0])
        self.variant = parts.count > 1 ? String(parts[1]) : nil
    }

    public static let claude = WaveModel("claude")
    public static let claudeOpus = WaveModel("claude:opus")
    public static let claudeSonnet = WaveModel("claude:sonnet")
    public static let codex = WaveModel("codex")
    public static let codexO3 = WaveModel("codex:o3")
    public static let codexO4Mini = WaveModel("codex:o4-mini")
    public static let gemini = WaveModel("gemini")

    /// Common models shown in the picker.
    public static let commonModels: [WaveModel] = [
        .claude,
        .claudeOpus,
        .claudeSonnet,
        .codex,
        .codexO3,
        .codexO4Mini,
        .gemini
    ]
}
