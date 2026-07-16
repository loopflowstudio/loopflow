import Foundation

public enum SessionOutcome: String, Codable, Sendable, CaseIterable, Hashable {
    case running, completed, failed, interrupted
}

public enum CaptureState: String, Codable, Sendable, CaseIterable, Hashable {
    case capturing, complete, partial
    case promptOnly = "prompt_only"
}

public enum ContextAssetKind: String, Codable, Sendable, Equatable {
    case operatingInstructions = "operating_instructions"
    case surfaceInstructions = "surface_instructions"
    case providerInstructions = "provider_instructions"
    case repoInstructions = "repo_instructions"
    case skillInstructions = "skill_instructions"
    case direction, goal, memory, chat, summary, document, scratch, diff, clipboard
    case userMessage = "user_message"
    case assembly
}

public struct SessionSetQuery: Codable, Hashable, Sendable {
    public var repoPaths: [String]
    public var startedAfter: Int64
    public var startedBefore: Int64
    public var waves: [String]
    public var projects: [String]
    public var tasks: [String]
    public var flows: [String]
    public var skills: [String]
    public var providers: [String]
    public var models: [String]
    public var surfaces: [String]
    public var outcomes: [SessionOutcome]
    public var captureStates: [CaptureState]
    public var steeredOnly: Bool
    public var currentRevisionOnly: Bool

    public init(
        repoPaths: [String],
        startedAfter: Int64,
        startedBefore: Int64,
        waves: [String],
        projects: [String],
        tasks: [String],
        flows: [String],
        skills: [String],
        providers: [String],
        models: [String],
        surfaces: [String],
        outcomes: [SessionOutcome],
        captureStates: [CaptureState],
        steeredOnly: Bool,
        currentRevisionOnly: Bool
    ) {
        self.repoPaths = repoPaths
        self.startedAfter = startedAfter
        self.startedBefore = startedBefore
        self.waves = waves
        self.projects = projects
        self.tasks = tasks
        self.flows = flows
        self.skills = skills
        self.providers = providers
        self.models = models
        self.surfaces = surfaces
        self.outcomes = outcomes
        self.captureStates = captureStates
        self.steeredOnly = steeredOnly
        self.currentRevisionOnly = currentRevisionOnly
    }

    enum CodingKeys: String, CodingKey {
        case waves, projects, tasks, flows, skills, providers, models, surfaces, outcomes
        case repoPaths = "repo_paths"
        case startedAfter = "started_after"
        case startedBefore = "started_before"
        case captureStates = "capture_states"
        case steeredOnly = "steered_only"
        case currentRevisionOnly = "current_revision_only"
    }
}

public struct ContextLabSnapshot: Decodable, Sendable {
    public let query: SessionSetQuery
    public let coverage: ContextCoverageSnapshot
    public let totals: SessionSetTotals
    public let aggregateRoot: ContextFlameNode
    public let sessions: [SessionLane]
    public let sources: [InstructionSourceSummary]
    public let evidence: [SourceEvidence]

    enum CodingKeys: String, CodingKey {
        case query, coverage, totals, sessions, sources, evidence
        case aggregateRoot = "aggregate_root"
    }
}

public struct ContextCoverageSnapshot: Decodable, Sendable {
    public let completeLaunches: UInt64
    public let partialLaunches: UInt64
    public let promptOnlyLaunches: UInt64
    public let capturingLaunches: UInt64
    public let attributedTurns: UInt64
    public let providerTotalOnlyTurns: UInt64
    public let unknownTurns: UInt64
    public let promptArtifactsAvailable: UInt64
    public let conversationsAvailable: UInt64
    public let sourceObservableAgentSessions: UInt64

    enum CodingKeys: String, CodingKey {
        case completeLaunches = "complete_launches"
        case partialLaunches = "partial_launches"
        case promptOnlyLaunches = "prompt_only_launches"
        case capturingLaunches = "capturing_launches"
        case attributedTurns = "attributed_turns"
        case providerTotalOnlyTurns = "provider_total_only_turns"
        case unknownTurns = "unknown_turns"
        case promptArtifactsAvailable = "prompt_artifacts_available"
        case conversationsAvailable = "conversations_available"
        case sourceObservableAgentSessions = "source_observable_agent_sessions"
    }
}

public struct SessionSetTotals: Decodable, Sendable {
    public let runs: UInt64
    public let agentSessions: UInt64
    public let turns: UInt64
    public let initialPromptTokens: UInt64?
    public let initialPromptAgentSessions: UInt64
    public let medianInitialPromptTokens: UInt64?
    public let p95InitialPromptTokens: UInt64?
    public let instructionTokens: UInt64?
    public let lifetimeInputTokens: UInt64?
    public let lifetimeInputAgentSessions: UInt64
    public let medianLifetimeInputTokens: UInt64?
    public let p95LifetimeInputTokens: UInt64?
    public let medianPeakContextPercent: Double?
    public let p95PeakContextPercent: Double?
    public let peakContextAgentSessions: UInt64
    public let completedLaunches: UInt64
    public let failedLaunches: UInt64
    public let interruptedLaunches: UInt64
    public let runningLaunches: UInt64
    public let steeringTurns: UInt64
    public let steeredLaunches: UInt64

    enum CodingKeys: String, CodingKey {
        case runs, turns
        case agentSessions = "agent_sessions"
        case initialPromptTokens = "initial_prompt_tokens"
        case initialPromptAgentSessions = "initial_prompt_agent_sessions"
        case medianInitialPromptTokens = "median_initial_prompt_tokens"
        case p95InitialPromptTokens = "p95_initial_prompt_tokens"
        case instructionTokens = "instruction_tokens"
        case lifetimeInputTokens = "lifetime_input_tokens"
        case lifetimeInputAgentSessions = "lifetime_input_agent_sessions"
        case medianLifetimeInputTokens = "median_lifetime_input_tokens"
        case p95LifetimeInputTokens = "p95_lifetime_input_tokens"
        case medianPeakContextPercent = "median_peak_context_percent"
        case p95PeakContextPercent = "p95_peak_context_percent"
        case peakContextAgentSessions = "peak_context_agent_sessions"
        case completedLaunches = "completed_launches"
        case failedLaunches = "failed_launches"
        case interruptedLaunches = "interrupted_launches"
        case runningLaunches = "running_launches"
        case steeringTurns = "steering_turns"
        case steeredLaunches = "steered_launches"
    }
}

public enum ContextFlameLevel: String, Decodable, Sendable {
    case sessionSet = "session_set"
    case kind, source, revision
}

public struct ContextFlameNode: Decodable, Identifiable, Sendable {
    public let id: String
    public let level: ContextFlameLevel
    public let kind: ContextAssetKind?
    public let label: String
    public let sourcePath: String?
    public let contentSha256: String?
    public let attributedTokens: UInt64
    public let runCount: UInt64
    public let agentSessionCount: UInt64
    public let turnCount: UInt64
    public let children: [ContextFlameNode]

    enum CodingKeys: String, CodingKey {
        case id, level, kind, label, children
        case sourcePath = "source_path"
        case contentSha256 = "content_sha256"
        case attributedTokens = "attributed_tokens"
        case runCount = "run_count"
        case agentSessionCount = "agent_session_count"
        case turnCount = "turn_count"
    }
}

public struct SessionLane: Decodable, Identifiable, Sendable {
    public let id: String
    public let runId: String
    public let startedAt: Int64
    public let outcome: SessionOutcome
    public let steeringTurns: UInt64?
    public let lifetimeInputTokens: UInt64?
    public let peakContextPercent: Double?
    public let provider: String
    public let model: String?
    public let surface: String
    public let wave: String?
    public let project: String?
    public let task: String?
    public let flow: String?
    public let skill: String?
    public let turns: [TurnLane]

    enum CodingKeys: String, CodingKey {
        case id, outcome, provider, model, surface, wave, project, task, flow, skill, turns
        case runId = "run_id"
        case startedAt = "started_at"
        case steeringTurns = "steering_turns"
        case lifetimeInputTokens = "lifetime_input_tokens"
        case peakContextPercent = "peak_context_percent"
    }
}

public struct TurnLane: Decodable, Identifiable, Sendable {
    public let id: String
    public let ordinal: UInt64
    public let suppliedContextTokens: UInt64?
    public let assets: [ContextLaneAsset]

    enum CodingKeys: String, CodingKey {
        case id, ordinal, assets
        case suppliedContextTokens = "supplied_context_tokens"
    }
}

public struct ContextLaneAsset: Decodable, Sendable {
    public let nodeId: String
    public let kind: ContextAssetKind
    public let label: String
    public let attributedTokens: UInt64

    enum CodingKeys: String, CodingKey {
        case kind, label
        case nodeId = "node_id"
        case attributedTokens = "attributed_tokens"
    }
}

public struct InstructionSourceSummary: Decodable, Identifiable, Sendable {
    public let id: String
    public let label: String
    public let kind: ContextAssetKind
    public let sourcePath: String
    public let impressions: UInt64?
    public let observedRevisions: UInt64
    public let lastSeen: Int64?
    public let currentRevisionNodeId: String?

    enum CodingKeys: String, CodingKey {
        case id, label, kind, impressions
        case sourcePath = "source_path"
        case observedRevisions = "observed_revisions"
        case lastSeen = "last_seen"
        case currentRevisionNodeId = "current_revision_node_id"
    }
}

public struct TraceAddress: Codable, Hashable, Sendable, Identifiable {
    public var id: String { "\(runId)/\(launchId)/\(turnId)" }

    public let runId: String
    public let launchId: String
    public let turnId: String

    public init(runId: String, launchId: String, turnId: String) {
        self.runId = runId
        self.launchId = launchId
        self.turnId = turnId
    }

    enum CodingKeys: String, CodingKey {
        case runId = "run_id"
        case launchId = "launch_id"
        case turnId = "turn_id"
    }
}

public enum EvidenceRole: String, Decodable, Sendable {
    case smoothComplete = "smooth_complete"
    case highContextComplete = "high_context_complete"
    case failedOrSteered = "failed_or_steered"
    case recent
}

public struct RepresentativeTrace: Decodable, Sendable {
    public let role: EvidenceRole
    public let address: TraceAddress
    public let outcome: SessionOutcome
    public let suppliedContextTokens: UInt64?
    public let selectedSourceTokens: UInt64
    public let promptArtifactAvailable: Bool
    public let conversationAvailable: Bool

    enum CodingKeys: String, CodingKey {
        case role, address, outcome
        case suppliedContextTokens = "supplied_context_tokens"
        case selectedSourceTokens = "selected_source_tokens"
        case promptArtifactAvailable = "prompt_artifact_available"
        case conversationAvailable = "conversation_available"
    }
}

public struct SourceMeasurements: Codable, Sendable {
    public let exposedSessions: UInt64
    public let exposedLaunches: UInt64
    public let exposedTurns: UInt64
    public let attributedTokens: UInt64
    public let medianTokensPerExposedTurn: UInt64?
    public let p95TokensPerExposedTurn: UInt64?
    public let firstSeen: Int64?
    public let lastSeen: Int64?
    public let completedLaunches: UInt64
    public let failedLaunches: UInt64
    public let steeringTurns: UInt64?
    public let completeCaptureLaunches: UInt64
    public let providerModels: [ProviderModelExposure]

    enum CodingKeys: String, CodingKey {
        case exposedSessions = "exposed_sessions"
        case exposedLaunches = "exposed_launches"
        case exposedTurns = "exposed_turns"
        case attributedTokens = "attributed_tokens"
        case medianTokensPerExposedTurn = "median_tokens_per_exposed_turn"
        case p95TokensPerExposedTurn = "p95_tokens_per_exposed_turn"
        case firstSeen = "first_seen"
        case lastSeen = "last_seen"
        case completedLaunches = "completed_launches"
        case failedLaunches = "failed_launches"
        case steeringTurns = "steering_turns"
        case completeCaptureLaunches = "complete_capture_launches"
        case providerModels = "provider_models"
    }
}

public struct ProviderModelExposure: Codable, Hashable, Sendable {
    public let provider: String
    public let model: String?
    public let exposedLaunches: UInt64

    public init(provider: String, model: String?, exposedLaunches: UInt64) {
        self.provider = provider
        self.model = model
        self.exposedLaunches = exposedLaunches
    }

    enum CodingKeys: String, CodingKey {
        case provider, model
        case exposedLaunches = "exposed_launches"
    }
}

public struct SourceEvidence: Decodable, Sendable, Identifiable {
    public var id: String { nodeId }
    public var isEditable: Bool {
        sourcePath != nil && currentContentSha256 == contentSha256
    }

    public let nodeId: String
    public let label: String
    public let kind: ContextAssetKind
    public let sourcePath: String?
    public let contentSha256: String
    public let currentContentSha256: String?
    public let currentSourceSha256: String?
    public let precedenceLayers: [String]
    public let measurements: SourceMeasurements
    public let representatives: [RepresentativeTrace]

    enum CodingKeys: String, CodingKey {
        case label, kind, measurements, representatives
        case nodeId = "node_id"
        case sourcePath = "source_path"
        case contentSha256 = "content_sha256"
        case currentContentSha256 = "current_content_sha256"
        case currentSourceSha256 = "current_source_sha256"
        case precedenceLayers = "precedence_layers"
    }
}

public struct RefinementSeed: Codable, Sendable {
    public let query: SessionSetQuery
    public let selectedNodeId: String
    public let sourcePath: String
    public let startingContentSha256: String
    public let measurements: SourceMeasurements
    public let evidence: [TraceAddress]

    public init(
        query: SessionSetQuery,
        selectedNodeId: String,
        sourcePath: String,
        startingContentSha256: String,
        measurements: SourceMeasurements,
        evidence: [TraceAddress]
    ) {
        self.query = query
        self.selectedNodeId = selectedNodeId
        self.sourcePath = sourcePath
        self.startingContentSha256 = startingContentSha256
        self.measurements = measurements
        self.evidence = evidence
    }

    enum CodingKeys: String, CodingKey {
        case query, measurements, evidence
        case selectedNodeId = "selected_node_id"
        case sourcePath = "source_path"
        case startingContentSha256 = "starting_content_sha256"
    }
}

public struct TraceContentSnapshot: Decodable, Sendable {
    public let address: TraceAddress
    public let systemPrompt: TraceArtifactSnapshot
    public let taskPrompt: TraceArtifactSnapshot
    public let conversation: TraceArtifactSnapshot

    enum CodingKeys: String, CodingKey {
        case address, conversation
        case systemPrompt = "system_prompt"
        case taskPrompt = "task_prompt"
    }
}

public struct TraceArtifactSnapshot: Decodable, Sendable {
    public let path: String?
    public let content: String?
    public let unavailableReason: String?

    enum CodingKeys: String, CodingKey {
        case path, content
        case unavailableReason = "unavailable_reason"
    }
}
