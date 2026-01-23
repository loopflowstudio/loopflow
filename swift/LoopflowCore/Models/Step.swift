// Step model - the basic unit of execution.
// A Step is a prompt to run with optional config.

import Foundation

/// Per-step configuration overrides.
public struct StepConfig: Sendable, Codable, Equatable {
    public var model: String?
    public var voice: String?
    public var context: [String]?

    public init(model: String? = nil, voice: String? = nil, context: [String]? = nil) {
        self.model = model
        self.voice = voice
        self.context = context
    }

    public var isEmpty: Bool {
        model == nil && voice == nil && (context == nil || context!.isEmpty)
    }
}

/// A Step is the basic unit - a prompt to run with optional config.
public struct Step: Sendable, Codable, Equatable, Identifiable {
    public var id: UUID = UUID()
    public var prompt: String
    public var config: StepConfig?

    enum CodingKeys: String, CodingKey {
        case prompt, config
    }

    public init(prompt: String, config: StepConfig? = nil) {
        self.prompt = prompt
        self.config = config
    }

    public init(from decoder: Decoder) throws {
        // Handle string shorthand: "design" -> Step(prompt: "design")
        if let singleValue = try? decoder.singleValueContainer(),
           let promptName = try? singleValue.decode(String.self) {
            self.prompt = promptName
            return
        }

        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.prompt = try container.decode(String.self, forKey: .prompt)
        self.config = try container.decodeIfPresent(StepConfig.self, forKey: .config)
    }

    public var hasConfig: Bool {
        guard let config = config else { return false }
        return !config.isEmpty
    }
}

/// A FlowStep is a Step with its parent Flow context.
public struct FlowStep: Sendable, Codable, Equatable, Identifiable {
    public var id: UUID = UUID()
    public var step: Step
    public var flow: String

    public init(step: Step, flow: String) {
        self.step = step
        self.flow = flow
    }
}

// MARK: - Step Execution

/// StepRun represents a single execution of a step.
/// Schema matches Python's lfd/models.py StepRun class.
public struct StepRun: Sendable, Identifiable, Codable, Hashable {
    public let id: String
    public let step: String  // matches Python schema (was "prompt")
    public let repo: String
    public let worktree: String
    public let status: String
    public let startedAt: Date
    public let endedAt: Date?
    public let model: String
    public let runMode: String

    enum CodingKeys: String, CodingKey {
        case id, step, repo, worktree, status, model
        case startedAt = "started_at"
        case endedAt = "ended_at"
        case runMode = "run_mode"
    }

    public var isRunning: Bool {
        status == "running" || status == "waiting"
    }

    public var isCompleted: Bool {
        status == "completed"
    }

    public var isError: Bool {
        status == "error"
    }

    public var relativeTime: String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: startedAt, relativeTo: Date())
    }
}
