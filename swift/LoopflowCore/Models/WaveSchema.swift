import Foundation

public struct WaveSchema: Sendable, Identifiable, Hashable {
    public enum Source: String, Sendable {
        case builtin
        case local
    }

    public let schemaRef: String
    public let name: String
    public let flow: String
    public let area: [String]
    public let direction: [String]
    public let owner: String?
    public let description: String?
    public let source: Source
    public let activeWaveId: String?
    public let stimulus: StimulusSchema?

    public init(
        schemaRef: String,
        name: String,
        flow: String,
        area: [String],
        direction: [String],
        owner: String?,
        description: String?,
        source: Source,
        activeWaveId: String?,
        stimulus: StimulusSchema?
    ) {
        self.schemaRef = schemaRef
        self.name = name
        self.flow = flow
        self.area = area
        self.direction = direction
        self.owner = owner
        self.description = description
        self.source = source
        self.activeWaveId = activeWaveId
        self.stimulus = stimulus
    }

    public var id: String { schemaRef }

    public var isInstantiated: Bool { activeWaveId != nil }
}

public struct StimulusSchema: Sendable, Hashable {
    public let kind: String
    public let cron: String?

    public init(kind: String, cron: String?) {
        self.kind = kind
        self.cron = cron
    }
}
