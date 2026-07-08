// Flow model - a sequence of skills.

import Foundation

public enum FlowType: String, Sendable, Codable {
    case flow
    case skill
}

/// A flow definition with name and skills.
public struct Flow: Sendable, Codable, Identifiable, Equatable {
    public var id: UUID = UUID()
    public var name: String
    public var skills: [Skill]
    public var type: FlowType

    enum CodingKeys: String, CodingKey {
        case name, skills, type
    }

    public init(name: String, skills: [Skill] = [], type: FlowType = .flow) {
        self.name = name
        self.skills = skills
        self.type = type
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)

        // Name comes from filename, not YAML content
        self.name = try container.decodeIfPresent(String.self, forKey: .name) ?? ""

        // Type defaults to flow
        self.type = try container.decodeIfPresent(FlowType.self, forKey: .type) ?? .flow

        // Skills can be strings or objects
        if let skillsContainer = try? container.nestedUnkeyedContainer(forKey: .skills) {
            var skills: [Skill] = []
            var mutableContainer = skillsContainer
            while !mutableContainer.isAtEnd {
                // Try decoding as string first
                if let promptName = try? mutableContainer.decode(String.self) {
                    skills.append(Skill(prompt: promptName))
                } else {
                    // Decode as full skill object
                    let skill = try mutableContainer.decode(Skill.self)
                    skills.append(skill)
                }
            }
            self.skills = skills
        } else {
            self.skills = []
        }
    }

}
