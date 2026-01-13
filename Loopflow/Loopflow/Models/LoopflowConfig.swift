// LoopflowConfig model for .lf/config.yaml.

import Foundation

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

    enum CodingKeys: String, CodingKey {
        case agentModel = "agent_model"
        case interactive, terminal, ide, workspace, context, exclude, push, pr, yolo
    }

    var terminalApp: TerminalApp {
        switch terminal?.lowercased() {
        case "warp": return .warp
        case "iterm": return .iterm
        case "kitty": return .kitty
        default: return .terminal
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
