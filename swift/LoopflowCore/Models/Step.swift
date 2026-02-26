// Step model - the basic unit of execution.
// A Step is a prompt to run with optional config.

import Foundation

/// Per-step configuration overrides.
public struct StepConfig: Sendable, Codable, Equatable {
    public var model: String?
    public var defaultModel: String?
    public var direction: String?
    public var context: [String]?

    public init(
        model: String? = nil,
        defaultModel: String? = nil,
        direction: String? = nil,
        context: [String]? = nil
    ) {
        self.model = model
        self.defaultModel = defaultModel
        self.direction = direction
        self.context = context
    }

    public var isEmpty: Bool {
        model == nil &&
            defaultModel == nil &&
            direction == nil &&
            (context == nil || context!.isEmpty)
    }

    enum CodingKeys: String, CodingKey {
        case model
        case defaultModel = "default_model"
        case direction
        case context
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

    /// Memberwise initializer (required since we add a custom init).
    public init(
        id: String,
        step: String,
        repo: String,
        worktree: String,
        status: String,
        startedAt: Date,
        endedAt: Date?,
        model: String,
        runMode: String
    ) {
        self.id = id
        self.step = step
        self.repo = repo
        self.worktree = worktree
        self.status = status
        self.startedAt = startedAt
        self.endedAt = endedAt
        self.model = model
        self.runMode = runMode
    }
}
