import Foundation
import Loopflow

enum ConversationScope: Hashable, Sendable {
    case repo(String)
    case wave(repo: String, id: String)
    case project(repo: String, id: String)
    case task(repo: String, id: String)

    var repoPath: String {
        switch self {
        case .repo(let path): path
        case .wave(let path, _), .project(let path, _), .task(let path, _): path
        }
    }

}

/// A human conversation captures the visible scope when Start is clicked.
/// Work binding, checkout selection, provider, and destination remain lf-owned.
struct ConversationLaunch: Equatable {
    let scope: ConversationScope

    var prompt: String {
        let purpose = switch scope {
        case .repo: "Help me shape an idea and find where it belongs in this repository."
        case .wave: "Help me advance this Wave: understand its direction and Projects, and shape the next useful work."
        case .project: "Help me move this Project forward: understand its intent and progress, and shape the next useful Task."
        case .task: "Help me work on this Task using its directive and current evidence."
        }
        return purpose + "\n\nA human is here for a conversation. Briefly orient to this scope, then ask what they want to work on. Opening this conversation does not request an autonomous operating pass, a new Task or worktree, or a message to anyone else."
    }

    func arguments(lf: String) -> [String] {
        let binding: [String] = switch scope {
        case .repo: []
        case .wave(_, let id): ["--wave", id]
        case .project(_, let id): ["--project", id]
        case .task(_, let id): ["--task", id]
        }
        return [lf, "--interactive"] + binding + [":", prompt]
    }
}

extension PodiumModel {
    var conversationScope: ConversationScope? {
        guard let repoPath else { return nil }
        guard navigation.content != .overview, let selection else { return .repo(repoPath) }
        switch selection.kind {
        case .wave:
            guard let name = wave(id: selection.id)?.wave.name
                ?? rosterWave(id: selection.id)?.api.name else { return nil }
            return .wave(repo: repoPath, id: name)
        case .project:
            guard let project = project(id: selection.id) else { return nil }
            return .project(repo: repoPath, id: project.project.project.id)
        case .task:
            guard let task = task(id: selection.id) else { return nil }
            return .task(repo: task.task.reference.workspace?.worktree ?? repoPath, id: task.task.task.identifier)
        }
    }

    var conversationLabel: String? {
        guard navigation.content != .overview, let selection else { return repoPath.map { URL(fileURLWithPath: $0).lastPathComponent } }
        switch selection.kind {
        case .wave: return wave(id: selection.id)?.wave.name ?? rosterWave(id: selection.id)?.api.name
        case .project: return project(id: selection.id)?.project.project.name
        case .task: return task(id: selection.id)?.task.task.identifier
        }
    }
}
