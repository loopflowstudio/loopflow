// Skill model - the basic unit of execution.
// A Skill is a prompt to run with optional config.

import Foundation

/// Per-skill configuration overrides.
public struct SkillConfig: Sendable, Codable, Equatable {
    public var agent: String?
    public var defaultAgent: String?
    public var direction: String?
    public var context: [String]?

    public init(
        agent: String? = nil,
        defaultAgent: String? = nil,
        direction: String? = nil,
        context: [String]? = nil
    ) {
        self.agent = agent
        self.defaultAgent = defaultAgent
        self.direction = direction
        self.context = context
    }

    public var isEmpty: Bool {
        agent == nil &&
            defaultAgent == nil &&
            direction == nil &&
            (context?.isEmpty ?? true)
    }

    enum CodingKeys: String, CodingKey {
        case agent
        case defaultAgent = "default_agent"
        case direction
        case context
    }
}

/// A Skill is the basic unit - a prompt to run with optional config.
public struct Skill: Sendable, Codable, Equatable, Identifiable {
    public var id: UUID = UUID()
    public var prompt: String
    public var config: SkillConfig?

    enum CodingKeys: String, CodingKey {
        case prompt, config
    }

    public init(prompt: String, config: SkillConfig? = nil) {
        self.prompt = prompt
        self.config = config
    }

    public init(from decoder: Decoder) throws {
        // Handle string shorthand: "design" -> Skill(prompt: "design")
        if let singleValue = try? decoder.singleValueContainer(),
           let promptName = try? singleValue.decode(String.self) {
            self.prompt = promptName
            return
        }

        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.prompt = try container.decode(String.self, forKey: .prompt)
        self.config = try container.decodeIfPresent(SkillConfig.self, forKey: .config)
    }

}
