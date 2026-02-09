import Foundation
import SwiftUI

public struct WaveViewModel: Sendable, Identifiable, Hashable {
    public var api: Wave
    public var worktreePath: String?
    public var branch: String?
    public var isDirty: Bool
    public var isRebasing: Bool
    public var isMerging: Bool
    public var hasDiff: Bool
    public var aheadMain: Int
    public var behindMain: Int
    public var aheadRemote: Int
    public var behindRemote: Int
    public var prURL: URL?
    public var prNumber: Int?
    public var prState: PRState?
    public var recentSteps: [StepRun]
    public var prLimit: Int
    public var mergeMode: MergeMode
    public var pid: Int?
    public var lastMainSha: String?
    public var waitingReason: WaitingReason?
    public var runStartedAt: Date?

    public init(
        api: Wave,
        worktreePath: String? = nil,
        branch: String? = nil,
        isDirty: Bool = false,
        isRebasing: Bool = false,
        isMerging: Bool = false,
        hasDiff: Bool = false,
        aheadMain: Int = 0,
        behindMain: Int = 0,
        aheadRemote: Int = 0,
        behindRemote: Int = 0,
        prURL: URL? = nil,
        prNumber: Int? = nil,
        prState: PRState? = nil,
        recentSteps: [StepRun] = [],
        prLimit: Int = 5,
        mergeMode: MergeMode = .pr,
        pid: Int? = nil,
        lastMainSha: String? = nil,
        waitingReason: WaitingReason? = nil,
        runStartedAt: Date? = nil
    ) {
        let activeRun = api.activeRun
        self.api = api
        self.worktreePath = worktreePath ?? activeRun?.worktree ?? api.localWorktree
        self.branch = branch ?? activeRun?.branch ?? api.remoteBranch
        self.isDirty = isDirty
        self.isRebasing = isRebasing
        self.isMerging = isMerging
        self.hasDiff = hasDiff
        self.aheadMain = aheadMain
        self.behindMain = behindMain
        self.aheadRemote = aheadRemote
        self.behindRemote = behindRemote
        self.prURL = prURL ?? activeRun?.pr?.url
        self.prNumber = prNumber ?? activeRun?.pr?.number
        self.prState = prState ?? activeRun?.pr?.state
        self.recentSteps = recentSteps
        self.prLimit = prLimit
        self.mergeMode = mergeMode
        self.pid = pid
        self.lastMainSha = lastMainSha
        self.waitingReason = waitingReason
        self.runStartedAt = runStartedAt ?? activeRun?.startedAt
    }

    public var id: String { api.id }

    public var name: String {
        get { api.name }
        set { api.name = newValue }
    }

    public var repo: String {
        get { api.repo }
        set { api.repo = newValue }
    }

    public var flow: String {
        get { api.flow }
        set { api.flow = newValue }
    }

    public var direction: [String] {
        get { api.direction }
        set { api.direction = newValue }
    }

    public var area: [String] {
        get { api.area }
        set { api.area = newValue }
    }

    public var stimulus: Stimulus {
        get { api.stimulus }
        set { api.stimulus = newValue }
    }

    public var status: WaveStatus {
        get { api.status }
        set { api.status = newValue }
    }

    public var iteration: Int {
        get { api.iteration }
        set { api.iteration = newValue }
    }

    public var activeRun: WaveRun? {
        get { api.activeRun }
        set { api.activeRun = newValue }
    }

    public var createdAt: Date? {
        get { api.createdAt }
        set { api.createdAt = newValue }
    }

    public var stepIndex: Int {
        activeRun?.stepIndex ?? 0
    }

    public var shortId: String { String(id.prefix(7)) }

    public var displayName: String {
        if !name.isEmpty { return name }
        let areaStr = area.first.map { $0 == "." ? "root" : $0 } ?? "root"
        return "\(areaStr) · \(flow.isEmpty ? "default" : flow)"
    }

    public var areaDisplay: String {
        if area.isEmpty { return "" }
        return area.first == "." ? "." : area.joined(separator: ", ")
    }

    public var directionDisplay: String {
        direction.isEmpty ? "" : direction.joined(separator: ", ")
    }

    public var commits: [CommitEntry] { api.commits }
    public var diffStat: String? { api.diffStat }
    public var flowSteps: [String] { api.flowSteps }

    public var statusText: String {
        switch status {
        case .running: return "Running"
        case .waiting: return "Waiting"
        case .idle: return "Idle"
        case .failed: return "Failed"
        case .paused: return "Paused"
        }
    }

    public var iterationText: String {
        iteration > 0 ? "iter \(iteration)" : ""
    }

    public var detailText: String {
        var parts: [String] = []
        if !areaDisplay.isEmpty { parts.append(areaDisplay) }
        if !flow.isEmpty { parts.append(flow) }
        if stimulus.kind != .manual { parts.append(stimulus.kind.rawValue) }
        return parts.joined(separator: " · ")
    }

    public var stimulusText: String {
        stimulus.description
    }

    public var statusIndicator: (icon: String, color: Color) {
        switch status {
        case .running:
            return ("circle.fill", .statusSuccess)
        case .waiting:
            return ("circle.lefthalf.filled", .statusWarning)
        case .failed:
            return ("xmark.circle.fill", .statusError)
        case .idle:
            if stimulus.kind == .cron {
                return ("clock", .gray)
            }
            return ("circle", .gray)
        case .paused:
            return ("pause.circle", .gray)
        }
    }

    public var pendingPR: (number: Int, url: URL?)? {
        guard let prNumber, prState == .open else { return nil }
        return (number: prNumber, url: prURL)
    }

    public var lastActivityAt: Date? {
        recentSteps.first?.endedAt ?? recentSteps.first?.startedAt
    }

    public var lastActivityDescription: String? {
        guard let step = recentSteps.first else { return nil }
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        let time = formatter.localizedString(for: step.endedAt ?? step.startedAt, relativeTo: Date())
        return "\(step.step) \(time)"
    }
}
