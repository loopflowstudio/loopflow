import Foundation

/// Source of a flow or skill definition.
public enum CatalogSource: String, Codable, Sendable, Hashable {
    case builtin
    case repo
}

/// A flow in the catalog: name, category, source, and the structured body
/// of items that make it up (skills, sub-flow refs, xor branches, loops, ops).
public struct CatalogFlow: Sendable, Codable, Hashable, Identifiable {
    public let name: String
    public let category: String
    public let source: CatalogSource
    public let items: [CatalogFlowItem]

    public var id: String { name }
}

/// A skill in the catalog.
public struct CatalogSkill: Sendable, Codable, Hashable, Identifiable {
    public let name: String
    public let category: String
    public let source: CatalogSource
    public let description: String?
    public let interactive: Bool?

    public var id: String { name }
}

/// One item inside a flow body. Mirrors `engine::flow::FlowItem` from Rust.
public enum CatalogFlowItem: Sendable, Codable, Hashable {
    case skill(name: String, interactive: Bool?)
    case op(command: String, args: [String])
    case flowRef(name: String)
    case xor(CatalogXor)
    case or(CatalogXor)
    case loop(CatalogLoop)
    case and(CatalogAnd)

    private enum CodingKeys: String, CodingKey {
        case type
        case data
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        switch type {
        case "Skill":
            let payload = try container.decode(SkillPayload.self, forKey: .data)
            self = .skill(name: payload.name, interactive: payload.interactive)
        case "Op":
            let payload = try container.decode(OpPayload.self, forKey: .data)
            self = .op(command: payload.command, args: payload.args ?? [])
        case "FlowRef":
            let name = try container.decode(String.self, forKey: .data)
            self = .flowRef(name: name)
        case "Xor":
            self = .xor(try container.decode(CatalogXor.self, forKey: .data))
        case "Or":
            self = .or(try container.decode(CatalogXor.self, forKey: .data))
        case "Loop":
            self = .loop(try container.decode(CatalogLoop.self, forKey: .data))
        case "And":
            self = .and(try container.decode(CatalogAnd.self, forKey: .data))
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .type,
                in: container,
                debugDescription: "Unknown flow item type \(type)"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .skill(name, interactive):
            try container.encode("Skill", forKey: .type)
            try container.encode(SkillPayload(name: name, interactive: interactive), forKey: .data)
        case let .op(command, args):
            try container.encode("Op", forKey: .type)
            try container.encode(OpPayload(command: command, args: args), forKey: .data)
        case let .flowRef(name):
            try container.encode("FlowRef", forKey: .type)
            try container.encode(name, forKey: .data)
        case let .xor(def):
            try container.encode("Xor", forKey: .type)
            try container.encode(def, forKey: .data)
        case let .or(def):
            try container.encode("Or", forKey: .type)
            try container.encode(def, forKey: .data)
        case let .loop(def):
            try container.encode("Loop", forKey: .type)
            try container.encode(def, forKey: .data)
        case let .and(def):
            try container.encode("And", forKey: .type)
            try container.encode(def, forKey: .data)
        }
    }

    private struct SkillPayload: Codable, Hashable {
        let name: String
        let interactive: Bool?
    }

    private struct OpPayload: Codable, Hashable {
        let command: String
        let args: [String]?
    }
}

public struct CatalogXor: Sendable, Codable, Hashable {
    public let router: String?
    public let paths: [String: CatalogXorPath]
}

public struct CatalogXorPath: Sendable, Codable, Hashable {
    public let flow: String?
    public let skill: String?
    public let description: String

    private enum CodingKeys: String, CodingKey {
        case flow, skill, description
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.flow = try container.decodeIfPresent(String.self, forKey: .flow)
        self.skill = try container.decodeIfPresent(String.self, forKey: .skill)
        self.description = try container.decodeIfPresent(String.self, forKey: .description) ?? ""
    }
}

public struct CatalogLoop: Sendable, Codable, Hashable {
    public let skills: [CatalogFlowItem]
    public let exit: CatalogXor
}

public struct CatalogAnd: Sendable, Codable, Hashable {
    public let branches: [CatalogFlowItem]
    public let synthesize: String?
}

/// Envelope for the flow catalog DTO.
public struct CatalogResponse: Sendable, Codable {
    public let ok: Bool
    public let result: Catalog
}

/// Whole catalog response.
public struct Catalog: Sendable, Codable, Hashable {
    public let flows: [CatalogFlow]
    public let skills: [CatalogSkill]

    public init(flows: [CatalogFlow], skills: [CatalogSkill]) {
        self.flows = flows
        self.skills = skills
    }

    public var flowsByName: [String: CatalogFlow] {
        Dictionary(uniqueKeysWithValues: flows.map { ($0.name, $0) })
    }

    public var skillsByName: [String: CatalogSkill] {
        Dictionary(uniqueKeysWithValues: skills.map { ($0.name, $0) })
    }
}

/// Compute "used by" — direct parents that reference this name in their body.
extension Catalog {
    public func directParents(of name: String) -> [CatalogFlow] {
        flows.filter { flow in flow.items.contains(where: { $0.references(name: name) }) }
            .sorted { $0.name < $1.name }
    }
}

extension CatalogFlowItem {
    /// Walk this item (recursively) and report whether it references the given
    /// skill or flow name.
    public func references(name: String) -> Bool {
        switch self {
        case let .skill(skillName, _):
            return skillName == name
        case let .op(command, _):
            return command == name
        case let .flowRef(refName):
            return refName == name
        case let .xor(def), let .or(def):
            if def.router == name { return true }
            return def.paths.values.contains { path in
                path.flow == name || path.skill == name
            }
        case let .loop(def):
            if def.exit.router == name { return true }
            if def.exit.paths.values.contains(where: { $0.flow == name || $0.skill == name }) {
                return true
            }
            return def.skills.contains { $0.references(name: name) }
        case let .and(def):
            return def.branches.contains { $0.references(name: name) }
        }
    }
}
