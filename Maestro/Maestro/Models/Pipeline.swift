// Pipeline model matching Python PipelineDef structures.

import Foundation

/// Per-step configuration overrides.
struct StepConfig: Codable, Equatable {
    var model: String?
    var voice: String?
    var context: [String]?

    var isEmpty: Bool {
        model == nil && voice == nil && (context == nil || context!.isEmpty)
    }
}

/// A single step in a pipeline.
struct PipelineStep: Codable, Equatable, Identifiable {
    var id: UUID = UUID()
    var task: String?
    var pipeline: String?  // Reference to nested pipeline
    var config: StepConfig?

    enum CodingKeys: String, CodingKey {
        case task, pipeline, config
    }

    init(task: String? = nil, pipeline: String? = nil, config: StepConfig? = nil) {
        self.task = task
        self.pipeline = pipeline
        self.config = config
    }

    init(from decoder: Decoder) throws {
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

    var displayName: String {
        if let task = task {
            return task
        }
        if let pipeline = pipeline {
            return "[\(pipeline)]"
        }
        return "?"
    }

    var hasConfig: Bool {
        guard let config = config else { return false }
        return !config.isEmpty
    }
}

/// A pipeline definition with name and steps.
struct PipelineDef: Codable, Identifiable, Equatable {
    var id: UUID = UUID()
    var name: String
    var steps: [PipelineStep]

    enum CodingKeys: String, CodingKey {
        case name, steps
    }

    init(name: String, steps: [PipelineStep] = []) {
        self.name = name
        self.steps = steps
    }

    init(from decoder: Decoder) throws {
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
    var stepCount: Int {
        steps.count
    }
}
