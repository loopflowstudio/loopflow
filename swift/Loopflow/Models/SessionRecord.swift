import Foundation

public enum SessionState: String, Codable, Sendable, Hashable {
    case waiting
    case active
    case ready
    case closed
}

public enum SessionKind: String, Codable, Sendable, Hashable {
    case ask
    case flow
    case interactive
}

public enum SessionActionKind: String, Codable, Sendable, Hashable {
    case open
    case moveHere = "move_here"
    case complete
}

/// Who chose a Session's title. Rust never lets a generated suggestion
/// replace a human-assigned name. `unavailable` means the canonical name lives
/// on another Home; the title is only a local label.
public enum SessionTitleSource: String, Codable, Sendable, Hashable {
    case generated
    case human
    case unavailable
}

/// Whether a Session is an occurrence of its Task's managed Flow. Rust owns
/// this from the Flow position or the Run's recorded capture; Swift never
/// infers it from Task, checkout, provider, or skill.
public enum SessionFlowMembership: Codable, Sendable, Hashable {
    case step(flow: String, invocationId: String, step: String, stepIndex: Int, iteration: Int,
              occurrence: SessionFlowOccurrence)
    case independent
    case unknown(reason: String)

    private enum Kind: String, Codable { case step, independent, unknown }

    enum CodingKeys: String, CodingKey {
        case kind, flow, step, iteration, occurrence, reason
        case invocationId = "invocation_id"
        case stepIndex = "step_index"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(Kind.self, forKey: .kind) {
        case .step:
            self = .step(
                flow: try container.decode(String.self, forKey: .flow),
                invocationId: try container.decode(String.self, forKey: .invocationId),
                step: try container.decode(String.self, forKey: .step),
                stepIndex: try container.decode(Int.self, forKey: .stepIndex),
                iteration: try container.decode(Int.self, forKey: .iteration),
                occurrence: try container.decode(SessionFlowOccurrence.self, forKey: .occurrence)
            )
        case .independent:
            self = .independent
        case .unknown:
            self = .unknown(reason: try container.decode(String.self, forKey: .reason))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .step(flow, invocationId, step, stepIndex, iteration, occurrence):
            try container.encode(Kind.step, forKey: .kind)
            try container.encode(flow, forKey: .flow)
            try container.encode(invocationId, forKey: .invocationId)
            try container.encode(step, forKey: .step)
            try container.encode(stepIndex, forKey: .stepIndex)
            try container.encode(iteration, forKey: .iteration)
            try container.encode(occurrence, forKey: .occurrence)
        case .independent:
            try container.encode(Kind.independent, forKey: .kind)
        case .unknown(let reason):
            try container.encode(Kind.unknown, forKey: .kind)
            try container.encode(reason, forKey: .reason)
        }
    }

    /// Short human label: "feature / implement · iteration 3", "Independent".
    public var label: String {
        switch self {
        case let .step(flow, _, step, _, iteration, occurrence):
            let label = iteration > 0 ? "\(flow) / \(step) · iteration \(iteration)" : "\(flow) / \(step)"
            switch occurrence {
            case .current: return label
            case .earlier: return "\(label) · earlier"
            case .past: return "\(label) · past run"
            }
        case .independent:
            return "Independent"
        case .unknown:
            return "Flow membership unknown"
        }
    }
}

/// Where a Flow occurrence sits relative to its Flow's current position.
public enum SessionFlowOccurrence: String, Codable, Sendable, Hashable {
    /// The invocation's current position.
    case current
    /// An earlier position of the invocation that is still active.
    case earlier
    /// An invocation that has finished or been replaced by a restart.
    case past
}

public struct SessionAction: Codable, Sendable, Hashable {
    public let kind: SessionActionKind
    public let label: String
    public let help: String
    public let unavailableReason: String?

    enum CodingKeys: String, CodingKey {
        case kind, label, help
        case unavailableReason = "unavailable_reason"
    }
}

/// One resumable Session and the exact command that opens it.
/// Rust owns completion, FlowStep decisions, and provider-client liveness.
public struct SessionRecord: Codable, Sendable, Hashable, Identifiable {
    public let id: String
    public let runId: String
    public let kind: SessionKind
    public let work: WorkReference?
    public let waveId: String?
    public let workPath: String?
    public let actions: [SessionAction]
    public let title: String
    public let titleSource: SessionTitleSource
    public let flowMembership: SessionFlowMembership
    public let detail: String
    public let cwd: String
    public let state: SessionState
    public let readySummary: String?
    public let openArgv: [String]
    public let terminalIds: [String]

    public func action(_ kind: SessionActionKind) -> SessionAction? {
        actions.first { $0.kind == kind }
    }

    enum CodingKeys: String, CodingKey {
        case id, kind, work, title, detail, cwd, state
        case waveId = "wave_id"
        case actions
        case runId = "run_id"
        case titleSource = "title_source"
        case flowMembership = "flow_membership"
        case workPath = "work_path"
        case readySummary = "ready_summary"
        case openArgv = "open_argv"
        case terminalIds = "terminal_ids"
    }
}
