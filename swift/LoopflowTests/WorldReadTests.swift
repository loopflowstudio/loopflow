#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

/// Every Desktop read decodes what the source-built `lf` prints against the
/// `portfolio` world, and reads something. A Rust wire change Swift cannot
/// decode fails the row that names its read.
@Suite("Desktop reads against a built world", .serialized)
struct WorldReadTests {
    @Test(.timeLimit(.minutes(3)), arguments: WorldRead.all)
    func readDecodesPopulated(_ read: WorldRead) async throws {
        try await read.check(try await World.shared("portfolio"))
    }

    /// A new public read without a row here fails by name.
    @Test func everyPublicReadHasAWorldRow() throws {
        let file = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("Loopflow/Services/RegistryQuery.swift")
        let source = try String(contentsOf: file, encoding: .utf8)
        let body = try #require(source.components(separatedBy: "public struct RegistryQuery: Sendable {")
            .dropFirst().first?.components(separatedBy: "// MARK: - Wire snapshots").first)
        let declaration = try Regex(#"public (?:static )?func (\w+)"#)
        let methods = Set(body.matches(of: declaration).compactMap { $0.output[1].substring.map(String.init) })
        let covered = Set(WorldRead.all.map(\.method)).union(WorldRead.excluded.keys)
        #expect(methods.subtracting(covered).sorted() == [], "RegistryQuery reads without a WorldRead row or exclusion")
        #expect(covered.subtracting(methods).sorted() == [], "WorldRead names methods RegistryQuery no longer has")
    }

    /// The `portfolio` world's desktop Wave through the projections the detail
    /// pane's Project and Task rows render (`WaveLens.forTasks` / `.forTask`,
    /// open-task count, KR list).
    @Test("the populated detail hierarchy renders objective, chapter KRs, Tasks, and shared lenses")
    func populatedDetailHierarchyProjectsThroughLensGrammar() async throws {
        let world = try await World.shared("portfolio")
        let workMap = try await world.query.status(wave: "desktop", cwd: world.repo.path).workMap

        // The Wave objective leads the pane; chapter KRs and Tasks share its scope.
        #expect(!workMap.objective.trimmingCharacters(in: .whitespaces).isEmpty)
        let chapter = try #require(workMap.currentProject)
        #expect(chapter.krs.map(\.text) == [
            "Opening a Task shows its conversation in under 200 ms",
            "Every Task state has a distinct row treatment",
        ])

        // One desktop Task landed; the other is still open.
        let tasks = workMap.tasks.items
        #expect(tasks.count == 2)
        #expect(tasks.filter { !$0.task.completed }.count == 1)

        // Task rows: the shared condition and reason, verbatim. Swift never
        // reconstructs the level from status or process flags.
        for task in tasks {
            let lens = WaveLens.forTask(task.condition)
            #expect(lens.color == WaveLensColor(task.condition.state))
            #expect(lens.reason == task.condition.reason)
            #expect(!lens.reason.isEmpty, "VoiceOver names every state")
        }

        // The Project row folds its Tasks' conditions; it invents none.
        let projectLens = WaveLens.forTasks(tasks: tasks)
        let source = try #require(tasks.first { $0.condition.reason == projectLens.reason })
        #expect(projectLens.color == WaveLensColor(source.condition.state))
    }
}

/// One RegistryQuery read and what the `portfolio` world must show through it.
struct WorldRead: Sendable, CustomTestStringConvertible {
    let name: String
    let method: String
    let check: @Sendable (World) async throws -> Void

    init(_ name: String, _ method: String, check: @escaping @Sendable (World) async throws -> Void) {
        self.name = name
        self.method = method
        self.check = check
    }

    var testDescription: String { name }

    // Names the `portfolio` recipe writes.
    static let running = "Open conversations faster"
    static let unstarted = "Copy the store for unreleased builds"
    static let untracked = "Untracked exploration"

    static let all: [WorldRead] = [
        WorldRead("portfolio", "portfolio") { world in
            let portfolio = try await world.query.portfolio()
            #expect(portfolio.repos.map(\.path.normalizedFilePath).contains(world.repo.path.normalizedFilePath))
        },
        WorldRead("repo identity", "repositoryIdentity") { world in
            let read = try await world.query.repositoryIdentity(cwd: world.repo.path)
            #expect(read.id.hasPrefix("repo_"))
        },
        WorldRead("repo identity --path", "repositoryPath") { world in
            let id = try await world.query.repositoryIdentity(cwd: world.repo.path).id
            let read = try await world.query.repositoryPath(id: id)
            #expect(read.normalizedFilePath == world.repo.path.normalizedFilePath)
        },
        WorldRead("wave list --all --current", "allWaves") { world in
            let read = try await world.query.allWaves()
            #expect(Set(read.map(\.name)) == ["desktop", "systems", "quality"])
        },
        WorldRead("wave list by repository path", "waves") { world in
            let read = try await world.query.waves(repoPath: world.repo.path)
            #expect(read.count == 3)
        },
        WorldRead("wave list", "repositoryWaves") { world in
            let read = try await world.query.repositoryWaves(cwd: world.repo.path)
            #expect(read.contains { $0.name == "desktop" })
        },
        WorldRead("wave status desktop", "status") { world in
            let detail = try await world.query.status(wave: "desktop", cwd: world.repo.path)
            #expect(detail.currentProject?.krs.count == 2)
            #expect(detail.tasks.items.count == 2)
            #expect(!detail.history.items.isEmpty)
            #expect(!detail.metricPortfolio.metrics.isEmpty)
        },
        WorldRead("wave status systems", "status") { world in
            let detail = try await world.query.status(wave: "systems", cwd: world.repo.path)
            #expect(detail.tasks.items.count == 2)
        },
        WorldRead("wave plan desktop", "plan") { world in
            let plan = try await world.query.plan(wave: "desktop", objective: "desktop", cwd: world.repo.path)
            #expect(!plan.projects.items.isEmpty)
        },
        WorldRead("wave show --all", "roadmap") { world in
            let roadmap = try await world.query.roadmap()
            #expect(roadmap.waves.count == 3)
            #expect(roadmap.tasks.items.count == 5)
        },
        WorldRead("wave show desktop", "roadmap") { world in
            let roadmap = try await world.query.roadmap(wave: "desktop")
            #expect(roadmap.waves.map(\.wave.name) == ["desktop"])
            #expect(roadmap.waves.first?.tasks.items.count == 2)
        },
        WorldRead("open Task link", "workLink") { world in
            let task = try await world.task(named: running)
            let link = try await world.query.workLink(URL(string: "loopflow://repo/task/\(task.identifier)")!)
            #expect(link.target.kind == .task)
            #expect(link.target.repositoryPath.normalizedFilePath == world.repo.path.normalizedFilePath)
        },
        WorldRead("open Wave link", "workLink") { world in
            let link = try await world.query.workLink(URL(string: "loopflow://repo/wave/desktop")!)
            #expect(link.target.kind == .wave)
        },
        WorldRead("wave show --task", "taskDestination") { world in
            let task = try await world.task(named: running)
            let destination = try await world.query.taskDestination(issue: task.identifier, repo: world.repo.path)
            #expect(destination.tasks.items.map(\.task.id) == [task.id])
        },
        WorldRead("task location", "taskLocation") { world in
            let task = try await world.task(named: running)
            let location = try await world.query.taskLocation(task: task.id, repository: task.repositoryId,
                                                              machine: try await world.query.localMachineId())
            #expect(location.taskID == task.id)
        },
        WorldRead("planning status", "peerPlanningStatus") { world in
            let read = try await world.query.peerPlanningStatus(cwd: world.repo.path)
            #expect(!read.isEmpty)
        },
        WorldRead("machine id", "localMachineId") { world in
            let read = try await world.query.localMachineId()
            #expect(read.hasPrefix("home_"))
        },
        WorldRead("config user", "userName") { world in
            let read = try await world.query.userName()
            #expect(read == "Demo")
        },
        WorldRead("monitor work --watch", "watchWork") { world in
            let task = try await world.task(named: running)
            let desktop = try await world.wave(named: "desktop")
            let observation = try await world.query.watchWork()
            observation.request(.scope(id: 1, WorkScope(
                repo: world.repo.path, headless: true, task: task.identifier, wave: desktop.id,
                activity: WorkActivityScope(wave: nil, project: nil, task: nil))))
            observation.request(.taskHistory(id: 2, task: task.identifier))
            var unread: Set = ["planning", "peer_planning", "sessions", "selections", "task",
                               "task_history", "wave", "work_activity", "activity"]
            for try await frame in observation.frames {
                if frame.hasBody { unread.remove(frame.content.part) }
                if unread.isEmpty { break }
            }
            await observation.cancel()
            #expect(unread.isEmpty, "Work parts never read with a body: \(unread.sorted())")
        },
        WorldRead("history", "workActivity") { world in
            let read = try await world.query.workActivity()
            #expect(!read.items.isEmpty)
        },
        WorldRead("task automation", "automationStatus") { world in
            let read = try await world.query.automationStatus(cwd: world.repo.path)
            #expect(read.tasks.count == 4)
        },
        WorldRead("task diff --files", "taskChanges") { world in
            let task = try await world.task(named: running)
            let changes = try await world.query.taskChanges(issue: task.identifier, cwd: world.repo.path)
            #expect(changes.files.map(\.path) == ["README.md"])
        },
        WorldRead("task diff", "taskDiff") { world in
            let task = try await world.task(named: running)
            let diff = try await world.query.taskDiff(issue: task.identifier, path: nil, cwd: world.repo.path)
            #expect(diff.patch.contains("paints its last turns first"))
        },
        WorldRead("task files", "taskFiles") { world in
            let task = try await world.task(named: running)
            let directory = try await world.query.taskFiles(issue: task.identifier, directory: "", cwd: world.repo.path)
            #expect(directory.entries.map(\.path).contains("README.md"))
        },
        WorldRead("task file", "taskFile") { world in
            let task = try await world.task(named: running)
            let file = try await world.query.taskFile(issue: task.identifier, path: "README.md", cwd: world.repo.path)
            #expect(file.content?.contains("paints its last turns first") == true)
        },
        WorldRead("graph list --processes --for-task", "taskGraphProcesses") { world in
            let task = try await world.task(named: running)
            let graphs = try await world.query.taskGraphProcesses(task: task.identifier, cwd: world.repo.path)
            #expect(graphs.count == 1)
            #expect(!(graphs.first?.steps.isEmpty ?? true))
        },
        WorldRead("graph list", "graphCatalog") { world in
            let read = try await world.query.graphCatalog(cwd: world.repo.path)
            #expect(read.contains { $0.compiled != nil })
        },
        WorldRead("wave workflow list", "workflowCatalog") { world in
            let read = try await world.query.workflowCatalog(cwd: world.repo.path)
            #expect(read.contains { $0.workflow != nil })
        },
        WorldRead("wave workflow source", "workflowSource") { world in
            let read = try await world.query.workflowCatalog(cwd: world.repo.path)
            let workflow = try #require(read.first)
            let source = try await world.query.workflowSource(workflow.name, wave: "desktop", cwd: world.repo.path)
            #expect(source.contains("edges"))
        },
        WorldRead("task comment", "taskComments") { world in
            let task = try await world.task(named: running)
            let comments = try await world.query.taskComments(id: task.identifier, cwd: world.repo.path)
            // Checkout also posts an ownership claim; find the person's comment among them.
            #expect(comments.comments.contains { $0.body.contains("Paint the last twenty turns first") })
        },
        WorldRead("usage --task", "taskHistory") { world in
            let task = try await world.task(named: running)
            let history = try await world.query.taskHistory(task: task.identifier, cwd: world.repo.path)
            #expect(!history.isEmpty)
            #expect(history.allSatisfy { $0.taskId == task.id })
        },
        WorldRead("usage --task --context", "taskContext") { world in
            let task = try await world.task(named: running)
            let read = try await world.query.taskContext(task: task.identifier, cwd: world.repo.path)
            #expect(!read.steps.isEmpty)
        },
        WorldRead("session list", "sessionPage") { world in
            let read = try await world.query.sessionPage(cwd: world.repo.path)
            #expect(read.entries.count == 3)
        },
        WorldRead("session list --interactive all", "sessionPage") { world in
            let read = try await world.query.sessionPage(includingHeadless: true, cwd: world.repo.path)
            #expect(read.entries.count >= 3)
        },
        WorldRead("session bind --dry-run", "previewSessionBinding") { world in
            let task = try await world.task(named: unstarted)
            let session = try await world.session(titled: untracked)
            let preview = try await world.query.previewSessionBinding(id: session.id, task: task.identifier,
                                                                      cwd: world.repo.path)
            #expect(preview.sessionId == session.id)
            #expect(preview.taskId == task.id)
        },
        WorldRead("repo new-chapter --dry-run", "realignProjects") { world in
            let desktop = try await world.wave(named: "desktop")
            let plan: [String: Any] = ["name": "Next", "waves": [[
                "wave_id": desktop.id, "successor_id": UUID().uuidString.lowercased(), "create": true,
                "project_name": "Desktop next", "krs": [["text": "Rows stay distinct", "holds": false]],
                "metric_targets": [] as [Any],
            ]]]
            let input = String(decoding: try JSONSerialization.data(withJSONObject: plan), as: UTF8.self)
            let preview = try await world.query.realignProjects(name: "Next", plan: input, preview: true,
                                                                cwd: world.repo.path)
            #expect(preview.waves.first?.predecessor?.name == "desktop")
            #expect(preview.waves.first?.tasks.isEmpty == false)
        },
        WorldRead("usage", "usage") { world in
            let read = try await world.query.usage(days: 30)
            #expect(read.count == 3)
        },
        WorldRead("repo tokens", "codebase") { world in
            let read = try await world.query.codebase(repoPath: world.repo.path)
            #expect(!read.children.isEmpty)
        },
        WorldRead("repo tokens --days", "codebaseHistory") { world in
            let read = try await world.query.codebaseHistory(repoPath: world.repo.path)
            #expect(!read.isEmpty)
        },
        WorldRead("doctor", "doctor") { world in
            let read = try await world.query.doctor()
            #expect(!read.checks.isEmpty)
        },
    ]

    /// Public members that are not world reads, and why.
    static let excluded: [String: String] = [
        "onMachine": "composes a remote transport over the same reads",
        "pinTaskLocation": "composes location validation over the same reads",
        "recording": "composes wire capture over the same reads",
        "decode": "the decoder every row exercises",
        "watchTaskFiles": "a file stream; TaskFilesTests observes it",
        "prepareSessionAttachment": "returns nil without a running Session; a world has none",
        "ensureProject": "writes", "setAutomation": "writes", "automateTask": "writes",
        "saveWorkflow": "writes", "customizeGraph": "writes", "runTask": "starts work",
        "resumeInterrupted": "starts work", "moveTask": "writes", "pauseTask": "writes",
        "completeTask": "writes", "reopenTask": "writes", "abandonTask": "writes",
        "updateTaskDirective": "writes", "saveTaskFile": "writes", "bindSession": "writes",
        "renameSession": "writes", "resetSession": "writes", "ensurePrimarySession": "writes",
        "prepareTaskSession": "records the Task's primary Session",
        "prepareSession": "records the Session's next input",
    ]
}

private extension World {
    func wave(named name: String) async throws -> Wave {
        try #require(try await query.allWaves().first { $0.name == name }, "\(name) is not in the world")
    }

    func task(named name: String) async throws -> TaskPlanningSnapshot {
        let roadmap = try await query.roadmap()
        return try #require(roadmap.tasks.items.first { $0.task.name == name }?.task,
                     "\(name) is not in the world")
    }

    func session(titled title: String) async throws -> SessionRecord {
        let page = try await query.sessionPage(cwd: repo.path)
        return try #require(page.entries.first { $0.title == title },
                     "\(title) is not in the world")
    }
}

private extension WorkFrame {
    var hasBody: Bool {
        switch content {
        case .planning(let body): body != nil
        case .peerPlanning(let body): body != nil
        case .sessions(let body): body != nil
        case .selections(let body): body != nil
        case .task(let body): body != nil
        case .taskHistory(let body): body != nil
        case .wave(let body): body != nil
        case .workActivity(let body): body != nil
        case .activity(let body): body != nil
        case .heartbeat, .checkpoint: false
        }
    }
}
#endif
