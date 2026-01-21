// Pipeline model matching Python PipelineDef structures.

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

/// A single step in a pipeline.
public struct PipelineStep: Sendable, Codable, Equatable, Identifiable {
    public var id: UUID = UUID()
    public var task: String?
    public var pipeline: String?  // Reference to nested pipeline
    public var config: StepConfig?

    enum CodingKeys: String, CodingKey {
        case task, pipeline, config
    }

    public init(task: String? = nil, pipeline: String? = nil, config: StepConfig? = nil) {
        self.task = task
        self.pipeline = pipeline
        self.config = config
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)

        // Handle string shorthand: "design" -> { task: "design" }
        if let singleValue = try? decoder.singleValueContainer(),
           let taskName = try? singleValue.decode(String.self) {
            self.task = taskName
            return
        }

        self.task = try container.decodeIfPresent(String.self, forKey: .task)
        self.pipeline = try container.decodeIfPresent(String.self, forKey: .pipeline)
        self.config = try container.decodeIfPresent(StepConfig.self, forKey: .config)
    }

    public var displayName: String {
        if let task = task {
            return task
        }
        if let pipeline = pipeline {
            return "[\(pipeline)]"
        }
        return "?"
    }

    public var hasConfig: Bool {
        guard let config = config else { return false }
        return !config.isEmpty
    }
}

/// A pipeline definition with name and steps.
public struct PipelineDef: Sendable, Codable, Identifiable, Equatable {
    public var id: UUID = UUID()
    public var name: String
    public var steps: [PipelineStep]

    enum CodingKeys: String, CodingKey {
        case name, steps
    }

    public init(name: String, steps: [PipelineStep] = []) {
        self.name = name
        self.steps = steps
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)

        // Name comes from filename, not YAML content
        self.name = try container.decodeIfPresent(String.self, forKey: .name) ?? ""

        // Steps can be strings or objects
        if let stepsContainer = try? container.nestedUnkeyedContainer(forKey: .steps) {
            var steps: [PipelineStep] = []
            var mutableContainer = stepsContainer
            while !mutableContainer.isAtEnd {
                // Try decoding as string first
                if let taskName = try? mutableContainer.decode(String.self) {
                    steps.append(PipelineStep(task: taskName))
                } else {
                    // Decode as full step object
                    let step = try mutableContainer.decode(PipelineStep.self)
                    steps.append(step)
                }
            }
            self.steps = steps
        } else {
            self.steps = []
        }
    }

    /// Number of tasks (for display).
    public var stepCount: Int {
        steps.count
    }
}
