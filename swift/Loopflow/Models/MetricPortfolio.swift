import Foundation

public struct MetricPortfolio: Decodable, Sendable, Hashable {
    public let metrics: [MetricReading]
    public let contractIssues: [MetricContractIssue]

    enum CodingKeys: String, CodingKey {
        case metrics
        case contractIssues = "contract_issues"
    }
}

public struct MetricIdentity: Decodable, Sendable, Hashable {
    public let waveId: String
    public let metricId: String

    enum CodingKeys: String, CodingKey {
        case waveId = "wave_id"
        case metricId = "metric_id"
    }
}

public enum MetricStage: String, Decodable, Sendable, Hashable {
    case installed, graduated
}

public enum MetricTarget: Decodable, Sendable, Hashable {
    case atLeast(Double)
    case atMost(Double)

    private enum CodingKeys: String, CodingKey { case kind, value }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let value = try container.decode(Double.self, forKey: .value)
        switch try container.decode(String.self, forKey: .kind) {
        case "at_least": self = .atLeast(value)
        case "at_most": self = .atMost(value)
        case let kind:
            throw DecodingError.dataCorruptedError(
                forKey: .kind,
                in: container,
                debugDescription: "unknown metric target '\(kind)'"
            )
        }
    }
}

public enum MetricFreshness: Decodable, Sendable, Hashable {
    case never
    case fresh(sourceTime: String, expiresAt: String)
    case stale(sourceTime: String, expiresAt: String)

    private enum CodingKeys: String, CodingKey {
        case kind
        case sourceTime = "source_time"
        case expiresAt = "expires_at"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "never": self = .never
        case "fresh":
            self = .fresh(
                sourceTime: try container.decode(String.self, forKey: .sourceTime),
                expiresAt: try container.decode(String.self, forKey: .expiresAt)
            )
        case "stale":
            self = .stale(
                sourceTime: try container.decode(String.self, forKey: .sourceTime),
                expiresAt: try container.decode(String.self, forKey: .expiresAt)
            )
        case let kind:
            throw DecodingError.dataCorruptedError(
                forKey: .kind,
                in: container,
                debugDescription: "unknown metric freshness '\(kind)'"
            )
        }
    }
}

public enum MetricEvidence: Decodable, Sendable, Hashable {
    case met(value: Double, sourceWindowStart: String, sourceWindowEnd: String)
    case missed(value: Double, sourceWindowStart: String, sourceWindowEnd: String)
    case unknown(MetricUnknownCause)
    case unavailable(reason: String, sourceAsOf: String)

    private enum CodingKeys: String, CodingKey {
        case kind, value, cause, reason
        case sourceWindowStart = "source_window_start"
        case sourceWindowEnd = "source_window_end"
        case sourceAsOf = "source_as_of"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "met":
            self = .met(
                value: try container.decode(Double.self, forKey: .value),
                sourceWindowStart: try container.decode(String.self, forKey: .sourceWindowStart),
                sourceWindowEnd: try container.decode(String.self, forKey: .sourceWindowEnd)
            )
        case "missed":
            self = .missed(
                value: try container.decode(Double.self, forKey: .value),
                sourceWindowStart: try container.decode(String.self, forKey: .sourceWindowStart),
                sourceWindowEnd: try container.decode(String.self, forKey: .sourceWindowEnd)
            )
        case "unknown":
            self = .unknown(try container.decode(MetricUnknownCause.self, forKey: .cause))
        case "unavailable":
            self = .unavailable(
                reason: try container.decode(String.self, forKey: .reason),
                sourceAsOf: try container.decode(String.self, forKey: .sourceAsOf)
            )
        case let kind:
            throw DecodingError.dataCorruptedError(
                forKey: .kind,
                in: container,
                debugDescription: "unknown metric evidence '\(kind)'"
            )
        }
    }
}

public enum MetricUnknownCause: Decodable, Sendable, Hashable {
    case never
    case revisionMismatch(expected: String, observed: String, sourceTime: String)
    case incomplete(value: Double, sourceWindowStart: String, sourceWindowEnd: String)
    case windowMismatch(value: Double, sourceWindowStart: String, sourceWindowEnd: String)
    case staleObservation(value: Double, sourceWindowStart: String, sourceWindowEnd: String)
    case staleUnavailable(reason: String, sourceAsOf: String)

    private enum CodingKeys: String, CodingKey {
        case kind, value, reason
        case expectedContractRevision = "expected_contract_revision"
        case observedContractRevision = "observed_contract_revision"
        case sourceTime = "source_time"
        case sourceWindowStart = "source_window_start"
        case sourceWindowEnd = "source_window_end"
        case sourceAsOf = "source_as_of"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "never": self = .never
        case "revision_mismatch":
            self = .revisionMismatch(
                expected: try container.decode(String.self, forKey: .expectedContractRevision),
                observed: try container.decode(String.self, forKey: .observedContractRevision),
                sourceTime: try container.decode(String.self, forKey: .sourceTime)
            )
        case "incomplete":
            self = .incomplete(
                value: try container.decode(Double.self, forKey: .value),
                sourceWindowStart: try container.decode(String.self, forKey: .sourceWindowStart),
                sourceWindowEnd: try container.decode(String.self, forKey: .sourceWindowEnd)
            )
        case "window_mismatch":
            self = .windowMismatch(
                value: try container.decode(Double.self, forKey: .value),
                sourceWindowStart: try container.decode(String.self, forKey: .sourceWindowStart),
                sourceWindowEnd: try container.decode(String.self, forKey: .sourceWindowEnd)
            )
        case "stale_observation":
            self = .staleObservation(
                value: try container.decode(Double.self, forKey: .value),
                sourceWindowStart: try container.decode(String.self, forKey: .sourceWindowStart),
                sourceWindowEnd: try container.decode(String.self, forKey: .sourceWindowEnd)
            )
        case "stale_unavailable":
            self = .staleUnavailable(
                reason: try container.decode(String.self, forKey: .reason),
                sourceAsOf: try container.decode(String.self, forKey: .sourceAsOf)
            )
        case let kind:
            throw DecodingError.dataCorruptedError(
                forKey: .kind,
                in: container,
                debugDescription: "unknown metric unknown cause '\(kind)'"
            )
        }
    }
}

public struct MetricReading: Decodable, Sendable, Hashable, Identifiable {
    public var id: MetricIdentity { identity }

    public let identity: MetricIdentity
    public let contractRevision: String
    public let name: String
    public let description: String
    public let projectId: String
    public let stage: MetricStage
    public let instrumented: Bool
    public let instrument: String
    public let unit: String
    public let target: MetricTarget
    public let window: String
    public let freshnessPolicy: String
    public let freshness: MetricFreshness
    public let evidence: MetricEvidence

    enum CodingKeys: String, CodingKey {
        case identity, name, description, stage, instrumented, instrument, unit, target, window, freshness, evidence
        case contractRevision = "contract_revision"
        case projectId = "project_id"
        case freshnessPolicy = "freshness_policy"
    }
}

public enum MetricContractIssue: Decodable, Sendable, Hashable {
    case malformedContract(path: String, message: String)
    case unresolvedOwner(waveId: String, metricId: String, projectId: String)
    case instrumentMismatch(
        waveId: String,
        metricId: String,
        contractInstrument: String,
        registeredInstrument: String
    )
    case invalidGraduation(
        waveId: String,
        metricId: String,
        contractRevision: String,
        reason: String
    )

    private enum CodingKeys: String, CodingKey {
        case kind, path, message, reason
        case waveId = "wave_id"
        case metricId = "metric_id"
        case projectId = "project_id"
        case contractInstrument = "contract_instrument"
        case registeredInstrument = "registered_instrument"
        case contractRevision = "contract_revision"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "malformed_contract":
            self = .malformedContract(
                path: try container.decode(String.self, forKey: .path),
                message: try container.decode(String.self, forKey: .message)
            )
        case "unresolved_owner":
            self = .unresolvedOwner(
                waveId: try container.decode(String.self, forKey: .waveId),
                metricId: try container.decode(String.self, forKey: .metricId),
                projectId: try container.decode(String.self, forKey: .projectId)
            )
        case "instrument_mismatch":
            self = .instrumentMismatch(
                waveId: try container.decode(String.self, forKey: .waveId),
                metricId: try container.decode(String.self, forKey: .metricId),
                contractInstrument: try container.decode(String.self, forKey: .contractInstrument),
                registeredInstrument: try container.decode(String.self, forKey: .registeredInstrument)
            )
        case "invalid_graduation":
            self = .invalidGraduation(
                waveId: try container.decode(String.self, forKey: .waveId),
                metricId: try container.decode(String.self, forKey: .metricId),
                contractRevision: try container.decode(String.self, forKey: .contractRevision),
                reason: try container.decode(String.self, forKey: .reason)
            )
        case let kind:
            throw DecodingError.dataCorruptedError(
                forKey: .kind,
                in: container,
                debugDescription: "unknown metric contract issue '\(kind)'"
            )
        }
    }
}
