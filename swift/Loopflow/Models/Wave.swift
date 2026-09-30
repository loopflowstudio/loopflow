/// A durable control plane for one repository. Projects and Tasks carry the
/// shipping state; a Wave itself has no worktree, branch, diff, or PR.
///
/// The app reads identity, placement and active Task counts from `WaveSnapshot`
/// (`lf wave list --json`). This is an app model, so defaults serve local call sites.
public struct Wave: Sendable, Identifiable, Hashable {
    public let id: String
    public let name: String
    public let repo: String
    public let status: WorkStatus
    public let activeTasks: Int
    public let parentWaveId: String?
    public let retiredAt: String?
    public let supersededByWaveId: String?
    public let retirementReason: String?

    public init(
        id: String,
        name: String,
        repo: String,
        status: WorkStatus,
        activeTasks: Int = 0,
        parentWaveId: String? = nil,
        retiredAt: String? = nil,
        supersededByWaveId: String? = nil,
        retirementReason: String? = nil
    ) {
        self.id = id
        self.name = name
        self.repo = repo
        self.status = status
        self.activeTasks = activeTasks
        self.parentWaveId = parentWaveId
        self.retiredAt = retiredAt
        self.supersededByWaveId = supersededByWaveId
        self.retirementReason = retirementReason
    }
}
