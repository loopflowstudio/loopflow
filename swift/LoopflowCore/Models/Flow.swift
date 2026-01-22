// Flow model - a sequence of steps.

import Foundation

/// A flow definition with name and steps.
public struct FlowDef: Sendable, Codable, Identifiable, Equatable {
    public var id: UUID = UUID()
    public var name: String
    public var steps: [Step]

    enum CodingKeys: String, CodingKey {
        case name, steps
    }

    public init(name: String, steps: [Step] = []) {
        self.name = name
        self.steps = steps
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)

        // Name comes from filename, not YAML content
        self.name = try container.decodeIfPresent(String.self, forKey: .name) ?? ""

        // Steps can be strings or objects
        if let stepsContainer = try? container.nestedUnkeyedContainer(forKey: .steps) {
            var steps: [Step] = []
            var mutableContainer = stepsContainer
            while !mutableContainer.isAtEnd {
                // Try decoding as string first
                if let promptName = try? mutableContainer.decode(String.self) {
                    steps.append(Step(prompt: promptName))
                } else {
                    // Decode as full step object
                    let step = try mutableContainer.decode(Step.self)
                    steps.append(step)
                }
            }
            self.steps = steps
        } else {
            self.steps = []
        }
    }

    /// Number of steps (for display).
    public var stepCount: Int {
        steps.count
    }
}
