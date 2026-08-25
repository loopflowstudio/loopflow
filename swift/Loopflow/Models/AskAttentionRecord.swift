import Foundation

public enum AskAttentionStateRecord: String, Codable, Sendable, Hashable {
    case queued, claimed, active, stale
    case notPresented = "not-presented"
}

public struct AskOriginRecord: Codable, Sendable, Hashable {
    public let work: WorkReference
}

public enum AskBodyRecord: Codable, Sendable, Hashable {
    case intervention(prompt: String)
    case flowStep(flow: String, nodeId: String, skill: String, iteration: UInt32)

    private enum CodingKeys: String, CodingKey {
        case kind, prompt, flow, skill, iteration
        case nodeId = "node_id"
    }

    private enum Kind: String, Codable {
        case intervention
        case flowStep = "flow_step"
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        switch try values.decode(Kind.self, forKey: .kind) {
        case .intervention:
            self = .intervention(prompt: try values.decode(String.self, forKey: .prompt))
        case .flowStep:
            self = .flowStep(
                flow: try values.decode(String.self, forKey: .flow),
                nodeId: try values.decode(String.self, forKey: .nodeId),
                skill: try values.decode(String.self, forKey: .skill),
                iteration: try values.decode(UInt32.self, forKey: .iteration)
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .intervention(let prompt):
            try values.encode(Kind.intervention, forKey: .kind)
            try values.encode(prompt, forKey: .prompt)
        case .flowStep(let flow, let nodeId, let skill, let iteration):
            try values.encode(Kind.flowStep, forKey: .kind)
            try values.encode(flow, forKey: .flow)
            try values.encode(nodeId, forKey: .nodeId)
            try values.encode(skill, forKey: .skill)
            try values.encode(iteration, forKey: .iteration)
        }
    }

    public var label: String {
        switch self {
        case .intervention(let prompt):
            prompt
        case .flowStep(_, _, let skill, _):
            "Review \(skill)"
        }
    }
}

public struct AskRecord: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let origin: AskOriginRecord
    public let request: AskBodyRecord
}

/// One generic Run claimed to answer an Ask, plus its exact attach route.
public struct AskSessionRecord: Codable, Sendable, Hashable, Identifiable {
    public let askId: String
    public let runId: String
    public let homeRoute: String
    public let attachArgv: [String]

    public var id: String { runId }

    enum CodingKeys: String, CodingKey {
        case askId = "ask_id"
        case runId = "run_id"
        case homeRoute = "home_route"
        case attachArgv = "attach_argv"
    }
}

/// `lf ask list --user --json`: a projection over Rust-owned Ask state.
/// Swift renders and refreshes this value; it never advances the lifecycle.
public struct AskAttentionRecord: Codable, Sendable, Hashable, Identifiable {
    public let ask: AskRecord
    public let surface: AskSessionRecord?
    public let attention: AskAttentionStateRecord

    public var id: String { ask.id }
}
