import Foundation

/// A machine-wide census of every live body —
/// Wave, Project Work, Task Work, direct execution, and interactive
/// invocation — grouped by Wave. It is a **pure projection** over three
/// already-merged contracts (`lf roadmap`, `lf runs`, `lf invocation list`),
/// indexed by the ids each contract declares. It performs no filesystem
/// inference and reconstructs no parentage; it reads the parent each row states.
///
/// The rules that live here, and nowhere in the view:
/// - **View-only by default.** Ordinary bodies expose no controls. Only an
///   active interactive invocation carries a `invocationId`, which re-attaches the exact
///   Invocation through `lf invocation attach`.
/// - **Blue propagation.** A invocation waiting on the user tints its Task, its
///   Project, and its Wave blue even while their bodies are alive.
/// - **Honest evidence.** Missing, stale, unreachable, stopped, and unavailable
///   stay distinguishable from a healthy empty state; a broken source never
///   renders as a quiet zero.

public enum AttentionTint: String, Sendable, Hashable {
    case green, red, blue, black, neutral
}

/// Why a row reads the way it does. Ordered by severity when several could
/// apply; a reading is never silently upgraded to `observed`.
public enum ActivityEvidence: String, Sendable, Hashable {
    case observed
    case stale
    case stopped
    case unreachable
    case missing
    case unavailable
}

public enum WorkActivityKind: String, Sendable, Hashable {
    case project
    case task
    case directExecution
    case invocation
}

/// One body in the census. Fields the contract does not carry are `nil`, never
/// invented — `model` and `step` are frequently absent and stay absent.
public struct WorkActivity: Sendable, Hashable, Identifiable {
    public let id: String
    public let kind: WorkActivityKind
    public let title: String
    public let subtitle: String?
    /// The row this one hangs under (a project/task row id), or `nil` at the top
    /// of a Wave. Read from the parent the source declares, never inferred.
    public let parentRowId: String?
    public let provider: String?
    public let model: String?
    public let home: String?
    public let worktree: String?
    public let step: String?
    public let ageSecs: Int?
    public let reason: String?
    public let nextOwner: WorkNextMoveOwner?
    public let tint: AttentionTint
    public let evidence: ActivityEvidence
    /// Set only when a Invocation exposes a User-openable attach route.
    public let invocationId: String?

    public var isOpenable: Bool { invocationId != nil }

    /// One spoken line: kind and identity, lens, ownership, reason, freshness,
    /// and the one available action. VoiceOver reads this instead of the visual
    /// tint so ownership and freshness are never colour-only.
    public var accessibilityLabel: String {
        var parts: [String] = ["\(kindNoun) \(title)"]
        if let subtitle, subtitle != title { parts.append(subtitle) }
        parts.append("\(tint.rawValue) lens")
        if let nextOwner { parts.append("waiting on \(nextOwner.rawValue)") }
        if let reason, !reason.isEmpty { parts.append(reason) }
        parts.append(freshnessPhrase)
        parts.append(isOpenable ? "Open available" : "View only")
        return parts.joined(separator: ", ") + "."
    }

    private var kindNoun: String {
        switch kind {
        case .project: "Project Work"
        case .task: "Task Work"
        case .directExecution: "Direct execution"
        case .invocation: "Interactive invocation"
        }
    }

    private var freshnessPhrase: String {
        let base: String
        switch evidence {
        case .observed: base = "observed"
        case .stale: base = "evidence stale"
        case .stopped: base = "body stopped"
        case .unreachable: base = "Home unreachable"
        case .missing: base = "no evidence"
        case .unavailable: base = "evidence unavailable"
        }
        if let ageSecs { return "\(base), \(ageSecs) seconds old" }
        return base
    }
}

/// Every live body under one Wave. `evidence`/`unavailableReason` describe the
/// Wave's roadmap reading: `unavailable` (the source failed) and `missing` (we
/// looked and found nothing live) are different facts and render differently.
public struct WaveActivity: Sendable, Hashable, Identifiable {
    public let id: String
    public let waveName: String
    public let home: String
    public let remote: Bool
    public let tint: AttentionTint
    public let evidence: ActivityEvidence
    public let unavailableReason: String?
    public let rows: [WorkActivity]
}

public struct WorkCensus: Sendable, Hashable {
    public let groups: [WaveActivity]

    /// A row is present in at least one group.
    public var isEmpty: Bool { groups.allSatisfy { $0.rows.isEmpty } }

    public init(
        roadmap: RoadmapSnapshot,
        runs: [SkillRunEntry],
        invocations: [InvocationSurfaceRecord],
        staleThresholdSecs: Int = 300
    ) {
        // Index the two side contracts by the identity each already carries:
        // invocations by their declared wave id, runs by their wave name. Only
        // active bodies enter the census.
        var invocationsByWave: [String: [InvocationSurfaceRecord]] = [:]
        for invocation in invocations where invocation.status.isActive {
            invocationsByWave[invocation.waveId, default: []].append(invocation)
        }
        var runsByWave: [String: [SkillRunEntry]] = [:]
        for run in runs where run.ended == nil {
            runsByWave[run.wave ?? "", default: []].append(run)
        }

        var groups: [WaveActivity] = []
        var seenWaveIds: Set<String> = []
        var seenWaveNames: Set<String> = []

        for waveRoadmap in roadmap.waves {
            let wave = waveRoadmap.wave
            seenWaveIds.insert(wave.id)
            seenWaveNames.insert(wave.name)
            let waveInvocations = invocationsByWave[wave.id] ?? []
            let waveRuns = runsByWave[wave.name] ?? []
            groups.append(
                Self.buildGroup(
                    wave: wave,
                    projects: waveRoadmap.projects,
                    runs: waveRuns,
                    invocations: waveInvocations,
                    staleThresholdSecs: staleThresholdSecs
                )
            )
        }

        // Nothing gets dropped: invocations and runs whose Wave is absent from the
        // roadmap still surface, under an explicit unattributed group.
        var orphanInvocations: [InvocationSurfaceRecord] = []
        for (waveId, list) in invocationsByWave where !seenWaveIds.contains(waveId) {
            orphanInvocations.append(contentsOf: list)
        }
        var orphanRuns: [SkillRunEntry] = []
        for (name, list) in runsByWave where !seenWaveNames.contains(name) {
            orphanRuns.append(contentsOf: list)
        }
        if !orphanInvocations.isEmpty || !orphanRuns.isEmpty {
            var rows = orphanRuns.map { Self.directExecutionRow($0) }
            rows.append(
                contentsOf: orphanInvocations.map {
                    Self.invocationRow($0, parentRowId: nil, staleThresholdSecs: staleThresholdSecs)
                }
            )
            let tint: AttentionTint = if rows.contains(where: { $0.tint == .red }) {
                .red
            } else if rows.contains(where: { $0.tint == .blue }) {
                .blue
            } else {
                .neutral
            }
            groups.append(
                WaveActivity(
                    id: "unattributed",
                    waveName: "Unattributed",
                    home: "",
                    remote: false,
                    tint: tint,
                    evidence: .observed,
                    unavailableReason: nil,
                    rows: rows
                )
            )
        }

        self.groups = groups
    }

    // MARK: - Group assembly

    private static func buildGroup(
        wave: WaveSnapshot,
        projects: WorkEvidence<RoadmapProject>,
        runs: [SkillRunEntry],
        invocations: [InvocationSurfaceRecord],
        staleThresholdSecs: Int
    ) -> WaveActivity {
        let remoteUnreachable = isRemote(wave.home) && !wave.live
        // Only User-routed attention paints the external attention queue blue.
        // Parent-routed Feedback remains on the parent's control lane.
        let userInvocations = invocations.filter {
            $0.attention?.kind == "user" && $0.attentionAt != nil
        }
        let blockingParentIds = Set(userInvocations.map(\.parentId))
        let waveLevelAttention = userInvocations.contains { $0.parentKind == "wave" }

        var rows: [WorkActivity] = []
        var anyRed = false
        var anyBlue = waveLevelAttention
        var handledInvocationIds: Set<String> = []

        if case let .available(projectItems, _) = projects {
            for project in projectItems {
                guard let runtime = project.runtime,
                    !isTerminal(runtime.status)
                else { continue }
                let projectRowId = runtime.workId
                var projectRed = false
                var projectBlue = blockingParentIds.contains(runtime.workId)
                var childRows: [WorkActivity] = []

                for task in project.tasks {
                    guard let taskRuntime = task.runtime,
                        !isTerminal(taskRuntime.status)
                    else { continue }
                    let taskRowId = taskRuntime.workId
                    let blockingInvocation = blockingParentIds.contains(taskRuntime.workId)
                    var tint = tint(for: task.attention.level)
                    if blockingInvocation && tint != .red { tint = .blue }
                    if tint == .red { projectRed = true }
                    if tint == .blue { projectBlue = true }
                    childRows.append(
                        WorkActivity(
                            id: taskRowId,
                            kind: .task,
                            title: task.task.identifier,
                            subtitle: task.task.name,
                            parentRowId: projectRowId,
                            provider: taskRuntime.provider,
                            model: nil,
                            home: wave.home.route,
                            worktree: task.reference.workspace?.worktree,
                            step: nil,
                            ageSecs: task.attention.evidenceAgeSeconds,
                            reason: task.attention.reason,
                            nextOwner: task.attention.nextOwner,
                            tint: tint,
                            evidence: taskEvidence(
                                task,
                                runtime: taskRuntime,
                                remoteUnreachable: remoteUnreachable,
                                staleThresholdSecs: staleThresholdSecs
                            ),
                            invocationId: nil
                        )
                    )
                    for invocation in invocations where invocation.parentId == taskRuntime.workId {
                        childRows.append(
                            invocationRow(
                                invocation,
                                parentRowId: taskRowId,
                                staleThresholdSecs: staleThresholdSecs
                            )
                        )
                        handledInvocationIds.insert(invocation.invocationId)
                    }
                }

                if projectRed { anyRed = true }
                if projectBlue { anyBlue = true }
                let projectTint: AttentionTint =
                    projectRed
                    ? .red : (projectBlue ? .blue : (runtime.processAlive ? .green : .black))
                rows.append(
                    WorkActivity(
                        id: projectRowId,
                        kind: .project,
                        title: project.project.name,
                        subtitle: project.project.slug,
                        parentRowId: nil,
                        provider: runtime.provider,
                        model: nil,
                        home: wave.home.route,
                        worktree: nil,
                        step: "iteration \(runtime.iteration)",
                        ageSecs: nil,
                        reason: runtime.reason,
                        nextOwner: project.nextMove.owner,
                        tint: projectTint,
                        evidence: projectEvidence(
                            runtime,
                            remoteUnreachable: remoteUnreachable
                        ),
                        invocationId: nil
                    )
                )
                rows.append(contentsOf: childRows)
                for invocation in invocations where invocation.parentId == runtime.workId {
                    rows.append(
                        invocationRow(
                            invocation,
                            parentRowId: projectRowId,
                            staleThresholdSecs: staleThresholdSecs
                        )
                    )
                    handledInvocationIds.insert(invocation.invocationId)
                }
            }
        }

        for run in runs {
            rows.append(directExecutionRow(run))
        }
        // Wave-level and orphaned invocations (parent not found in this roadmap).
        for invocation in invocations where !handledInvocationIds.contains(invocation.invocationId) {
            let row = invocationRow(
                invocation,
                parentRowId: nil,
                staleThresholdSecs: staleThresholdSecs
            )
            if row.tint == .red { anyRed = true }
            if row.tint == .blue { anyBlue = true }
            rows.append(row)
        }

        let evidence: ActivityEvidence
        let unavailableReason: String?
        switch projects {
        case .unavailable(let reason):
            evidence = .unavailable
            unavailableReason = reason
        case .available:
            evidence = rows.isEmpty ? .missing : .observed
            unavailableReason = nil
        }

        let tint: AttentionTint
        if anyRed {
            tint = .red
        } else if anyBlue {
            tint = .blue
        } else if rows.contains(where: { $0.tint == .green }) {
            tint = .green
        } else {
            tint = .neutral
        }

        return WaveActivity(
            id: wave.id,
            waveName: wave.name,
            home: wave.home.route,
            remote: isRemote(wave.home),
            tint: tint,
            evidence: evidence,
            unavailableReason: unavailableReason,
            rows: rows
        )
    }

    // MARK: - Row builders

    private static func invocationRow(
        _ invocation: InvocationSurfaceRecord,
        parentRowId: String?,
        staleThresholdSecs: Int
    ) -> WorkActivity {
        let userAttention = invocation.attention?.kind == "user" && invocation.attentionAt != nil
        // The Feedback presentation opens by Work and Invocation, so an empty argv no longer
        // makes a User-routed Feedback unopenable.
        let openable = userAttention
        return WorkActivity(
            id: "invocation:\(invocation.invocationId)",
            kind: .invocation,
            title: invocation.reason,
            subtitle: "\(invocation.parentKind) invocation · \(invocation.provider)",
            parentRowId: parentRowId,
            provider: invocation.provider,
            model: nil,
            home: invocation.home,
            worktree: nil,
            step: nil,
            ageSecs: invocation.ageSecs,
            reason: invocation.reason,
            nextOwner: userAttention ? .user : nil,
            tint: userAttention ? .blue : .green,
            evidence: isStale(invocation.ageSecs, staleThresholdSecs) ? .stale : .observed,
            invocationId: openable ? invocation.id : nil
        )
    }

    private static func directExecutionRow(_ run: SkillRunEntry) -> WorkActivity {
        WorkActivity(
            id: "run:\(run.execId)",
            kind: .directExecution,
            title: run.flow ?? run.skill,
            subtitle: run.flow == nil ? nil : run.skill,
            parentRowId: nil,
            provider: run.provider,
            model: run.model,
            home: nil,
            worktree: run.worktree,
            step: run.skill,
            ageSecs: nil,
            reason: run.status,
            nextOwner: nil,
            tint: .green,
            evidence: .observed,
            invocationId: nil
        )
    }

    // MARK: - Classification

    private static func tint(for level: TaskAttentionLevel) -> AttentionTint {
        switch level {
        case .green: .green
        case .red: .red
        case .blue: .blue
        case .black: .black
        case .unknown: .neutral
        }
    }

    private static func taskEvidence(
        _ task: RoadmapTask,
        runtime: TaskRuntimeSnapshot,
        remoteUnreachable: Bool,
        staleThresholdSecs: Int
    ) -> ActivityEvidence {
        if task.attention.process.state == .unavailable { return .unavailable }
        if !runtime.processAlive { return .stopped }
        if remoteUnreachable { return .unreachable }
        if isStale(task.attention.evidenceAgeSeconds, staleThresholdSecs) { return .stale }
        return .observed
    }

    private static func projectEvidence(
        _ runtime: ProjectRuntimeSnapshot,
        remoteUnreachable: Bool
    ) -> ActivityEvidence {
        if !runtime.processAlive { return .stopped }
        if remoteUnreachable { return .unreachable }
        return .observed
    }

    private static func isStale(_ ageSecs: Int?, _ threshold: Int) -> Bool {
        guard let ageSecs else { return false }
        return ageSecs > threshold
    }

    private static func isTerminal(_ status: WorkStatus) -> Bool {
        status == .done || status == .abandoned
    }

    private static func isRemote(_ home: Home) -> Bool {
        home.route != "local"
    }
}
