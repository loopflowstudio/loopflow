import Foundation

public enum InvocationOutcome: String, Codable, Sendable, CaseIterable, Hashable {
    case running, completed, failed, interrupted, unknown
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

public struct InvocationSetQuery: Codable, Hashable, Sendable {
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
    public var outcomes: [InvocationOutcome]
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
        outcomes: [InvocationOutcome],
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
    public let query: InvocationSetQuery
    public let coverage: ContextCoverageSnapshot
    public let totals: InvocationSetTotals
    public let aggregateRoot: ContextFlameNode
    public let invocations: [InvocationLane]
    public let sources: [InstructionSourceSummary]
    public let evidence: [SourceEvidence]

    enum CodingKeys: String, CodingKey {
        case query, coverage, totals, invocations, sources, evidence
        case aggregateRoot = "aggregate_root"
    }
}

public struct ContextCoverageSnapshot: Decodable, Sendable {
    public let completeInvocations: UInt64
    public let partialInvocations: UInt64
    public let promptOnlyInvocations: UInt64
    public let capturingInvocations: UInt64
    public let attributedTurns: UInt64
    public let providerTotalOnlyTurns: UInt64
    public let unknownTurns: UInt64
    public let promptArtifactsAvailable: UInt64
    public let conversationsAvailable: UInt64
    public let sourceObservableInvocations: UInt64

    enum CodingKeys: String, CodingKey {
        case completeInvocations = "complete_invocations"
        case partialInvocations = "partial_invocations"
        case promptOnlyInvocations = "prompt_only_invocations"
        case capturingInvocations = "capturing_invocations"
        case attributedTurns = "attributed_turns"
        case providerTotalOnlyTurns = "provider_total_only_turns"
        case unknownTurns = "unknown_turns"
        case promptArtifactsAvailable = "prompt_artifacts_available"
        case conversationsAvailable = "conversations_available"
        case sourceObservableInvocations = "source_observable_invocations"
    }
}

public struct InvocationSetTotals: Decodable, Sendable {
    public let runs: UInt64
    public let invocations: UInt64
    public let turns: UInt64
    public let initialPromptTokens: UInt64?
    public let initialPromptInvocations: UInt64
    public let medianInitialPromptTokens: UInt64?
    public let p95InitialPromptTokens: UInt64?
    public let instructionTokens: UInt64?
    public let lifetimeInputTokens: UInt64?
    public let lifetimeInputInvocations: UInt64
    public let medianLifetimeInputTokens: UInt64?
    public let p95LifetimeInputTokens: UInt64?
    public let medianPeakContextPercent: Double?
    public let p95PeakContextPercent: Double?
    public let peakContextInvocations: UInt64
    public let completedInvocations: UInt64
    public let failedInvocations: UInt64
    public let interruptedInvocations: UInt64
    public let runningInvocations: UInt64
    public let unverifiedInvocations: UInt64
    public let steeringTurns: UInt64
    public let steeredInvocations: UInt64

    enum CodingKeys: String, CodingKey {
        case runs, turns
        case invocations = "invocations"
        case initialPromptTokens = "initial_prompt_tokens"
        case initialPromptInvocations = "initial_prompt_invocations"
        case medianInitialPromptTokens = "median_initial_prompt_tokens"
        case p95InitialPromptTokens = "p95_initial_prompt_tokens"
        case instructionTokens = "instruction_tokens"
        case lifetimeInputTokens = "lifetime_input_tokens"
        case lifetimeInputInvocations = "lifetime_input_invocations"
        case medianLifetimeInputTokens = "median_lifetime_input_tokens"
        case p95LifetimeInputTokens = "p95_lifetime_input_tokens"
        case medianPeakContextPercent = "median_peak_context_percent"
        case p95PeakContextPercent = "p95_peak_context_percent"
        case peakContextInvocations = "peak_context_invocations"
        case completedInvocations = "completed_invocations"
        case failedInvocations = "failed_invocations"
        case interruptedInvocations = "interrupted_invocations"
        case runningInvocations = "running_invocations"
        case unverifiedInvocations = "unverified_invocations"
        case steeringTurns = "steering_turns"
        case steeredInvocations = "steered_invocations"
    }
}

public enum ContextFlameLevel: String, Decodable, Sendable {
    case invocationSet = "invocation_set"
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
    public let invocationCount: UInt64
    public let turnCount: UInt64
    public let children: [ContextFlameNode]

    enum CodingKeys: String, CodingKey {
        case id, level, kind, label, children
        case sourcePath = "source_path"
        case contentSha256 = "content_sha256"
        case attributedTokens = "attributed_tokens"
        case runCount = "run_count"
        case invocationCount = "invocation_count"
        case turnCount = "turn_count"
    }
}

public struct InvocationLane: Decodable, Identifiable, Sendable {
    public let id: String
    public let runId: String
    public let startedAt: Int64
    public let outcome: InvocationOutcome
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
    public var id: String { "\(runId)/\(invocationId)/\(turnId)" }

    public let runId: String
    public let invocationId: String
    public let turnId: String

    public init(runId: String, invocationId: String, turnId: String) {
        self.runId = runId
        self.invocationId = invocationId
        self.turnId = turnId
    }

    enum CodingKeys: String, CodingKey {
        case runId = "run_id"
        case invocationId = "invocation_id"
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
    public let outcome: InvocationOutcome
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
    public let exposedRuns: UInt64
    public let exposedInvocations: UInt64
    public let exposedTurns: UInt64
    public let attributedTokens: UInt64
    public let medianTokensPerExposedTurn: UInt64?
    public let p95TokensPerExposedTurn: UInt64?
    public let firstSeen: Int64?
    public let lastSeen: Int64?
    public let completedInvocations: UInt64
    public let failedInvocations: UInt64
    public let steeringTurns: UInt64?
    public let completeCaptureInvocations: UInt64
    public let providerModels: [ProviderModelExposure]

    enum CodingKeys: String, CodingKey {
        case exposedRuns = "exposed_runs"
        case exposedInvocations = "exposed_invocations"
        case exposedTurns = "exposed_turns"
        case attributedTokens = "attributed_tokens"
        case medianTokensPerExposedTurn = "median_tokens_per_exposed_turn"
        case p95TokensPerExposedTurn = "p95_tokens_per_exposed_turn"
        case firstSeen = "first_seen"
        case lastSeen = "last_seen"
        case completedInvocations = "completed_invocations"
        case failedInvocations = "failed_invocations"
        case steeringTurns = "steering_turns"
        case completeCaptureInvocations = "complete_capture_invocations"
        case providerModels = "provider_models"
    }
}

public struct ProviderModelExposure: Codable, Hashable, Sendable {
    public let provider: String
    public let model: String?
    public let exposedInvocations: UInt64

    public init(provider: String, model: String?, exposedInvocations: UInt64) {
        self.provider = provider
        self.model = model
        self.exposedInvocations = exposedInvocations
    }

    enum CodingKeys: String, CodingKey {
        case provider, model
        case exposedInvocations = "exposed_invocations"
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
    public let query: InvocationSetQuery
    public let selectedNodeId: String
    public let sourcePath: String
    public let startingContentSha256: String
    public let measurements: SourceMeasurements
    public let evidence: [TraceAddress]

    public init(
        query: InvocationSetQuery,
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
