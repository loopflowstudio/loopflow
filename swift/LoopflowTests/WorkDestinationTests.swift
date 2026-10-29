#if os(macOS)
import AppKit
import Foundation
import Observation
import Testing
import ViewInspector
import os
@testable import Loopflow
@testable import LoopflowMac

private final class RemoteCompanionBundleMarker: NSObject {}

@Suite("Work destinations", .serialized)
@MainActor
struct WorkDestinationTests {
    private func destinationModel(query: RegistryQuery, repoPath: String? = nil) -> (WorkModel, SessionsWorkspaceRegistry) {
        let model = WorkModel(query: query, repoPath: repoPath)
        let registry = SessionsWorkspaceRegistry(localMachineId: "machine-local")
        model.bindWorkspaces(registry, repository: "repo")
        return (model, registry)
    }

    private func installedSession(_ model: WorkModel, _ registry: SessionsWorkspaceRegistry) -> SessionRecord? {
        guard let key = registry.selectedWork, let workspace = registry.retained(key),
              let id = workspace.multiplexer.layout.allPanes.compactMap(\.sessionId).first else { return nil }
        return model.sessions.value?.first { $0.id == id }
    }

    @Test(arguments: [false, true])
    func taskEntryOpensConversationThenRetainsWorkspace(standalone: Bool) async throws {
        var data = try fixture()
        if standalone {
            var wire = try #require(JSONSerialization.jsonObject(with: Data(data.utf8)) as? [String: Any])
            var inventory = try #require(wire["tasks"] as? [String: Any])
            var task = try #require((inventory["items"] as? [[String: Any]])?.first)
            var summary = try #require(task["task"] as? [String: Any])
            summary["wave_id"] = NSNull()
            summary["project_id"] = NSNull()
            task["task"] = summary
            inventory["items"] = [task]
            wire["tasks"] = inventory
            wire["waves"] = []
            data = String(decoding: try JSONSerialization.data(withJSONObject: wire), as: UTF8.self)
        }
        let planning = data
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let task = try #require(snapshot.tasks.items.first)
        let record = try renameFixtureRecord("primary", title: "Task conversation", work: .task(id: #require(task.runtime?.workId)))
        let encoded = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let query = RegistryQuery { args, _ in
            if args.prefix(2) == ["session", "resume"] { return encoded }
            if args.starts(with: ["desktop", "plan"]) { return planning }
            throw RegistryQueryError("Task entry cannot enumerate unrelated Sessions, launch a Workflow or take over a client")
        }
        let (model, registry) = destinationModel(query: query, repoPath: task.task.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        await model.openTaskConversation(task.id)
        #expect(installedSession(model, registry)?.id == record.id)
        #expect(model.selection == .task(id: task.id))
        let entry = try #require(registry.retained(model.workspaceKey(for: model.selection, repository: "repo")))
        entry.multiplexer.newShell(command: ["retained-shell"], location: record.workspace?.identity)
        let files = entry.files(issue: task.task.identifier, cwd: "/fixture", query: query)
        let draft = files.document("retained.swift")
        draft.editor.string = "unsaved Task draft"
        let layout = entry.multiplexer.layout
        let focus = entry.multiplexer.focusedPaneId
        model.select(nil)
        await model.openTaskConversation(task.id)
        #expect(entry.multiplexer.layout == layout)
        #expect(entry.multiplexer.focusedPaneId == focus)
        #expect(files.document("retained.swift") === draft)
        #expect(draft.editor.string == "unsaved Task draft")
        #expect(model.navigation.content == .terminals)
    }

    @Test("New Task creates through one lf command, then shows its conversation")
    func newTaskSelectsTheTaskAndOpensItsConversation() async throws {
        let planning = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(planning.utf8))
        let task = try #require(snapshot.tasks.items.first)
        let record = try renameFixtureRecord("primary", title: "Task conversation", work: .task(id: #require(task.runtime?.workId)))
        let encoded = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let receipt = """
        {"task": {"id": "\(task.task.id)", "identifier": "\(task.task.identifier)", "title": "\(task.task.name)"},
         "graph": "code", "session_id": "\(record.id)"}
        """
        let created = OSAllocatedUnfairLock<[[String]]>(initialState: [])
        let query = RegistryQuery { args, _ in
            if args.starts(with: ["task", "create"]) {
                created.withLock { $0.append(args) }
                return receipt
            }
            if args.prefix(2) == ["session", "resume"] { return encoded }
            if args.starts(with: ["desktop", "plan"]) { return planning }
            throw RegistryQueryError("New Task runs no separate start or connect sequence")
        }
        let (model, registry) = destinationModel(query: query, repoPath: task.task.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        model.navigation.palette = .newTask
        try await model.createTask(request: "fix the flaky retry test", wave: nil, graph: nil)
        #expect(created.withLock { $0 } == [["task", "create", "--no-wave", "--run", "--json", "--", "fix the flaky retry test"]])
        #expect(model.navigation.palette == nil)
        #expect(model.selection == .task(id: task.id))
        #expect(installedSession(model, registry)?.id == record.id)
    }

    @Test("New Task selects the new Task before the window's other readings confirm")
    func newTaskSelectsBeforeReadingsConfirm() async throws {
        let planning = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(planning.utf8))
        let task = try #require(snapshot.tasks.items.first)
        let record = try renameFixtureRecord("primary", title: "Task conversation", work: .task(id: #require(task.runtime?.workId)))
        let encoded = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let receipt = """
        {"task": {"id": "\(task.task.id)", "identifier": "\(task.task.identifier)", "title": "\(task.task.name)"},
         "graph": "code", "session_id": "\(record.id)"}
        """
        let barrier = LinkedDestinationBarrier()
        let held = OSAllocatedUnfairLock(initialState: true)
        let query = RegistryQuery { args, _ in
            if args.starts(with: ["task", "create"]) { return receipt }
            if args.prefix(2) == ["session", "resume"] { return encoded }
            if args.starts(with: ["desktop", "plan"]) {
                if held.withLock({ $0 }) { await barrier.wait("planning") }
                return planning
            }
            throw RegistryQueryError("Unexpected New Task operation")
        }
        let (model, _) = destinationModel(query: query, repoPath: task.task.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        let creating = Task { try await model.createTask(request: "fix the flaky retry test", wave: nil, graph: nil) }
        let deadline = ContinuousClock.now + .seconds(3)
        while !(await barrier.contains("planning")), ContinuousClock.now < deadline { await Task.yield() }
        #expect(model.selection == .task(id: task.id))
        held.withLock { $0 = false }
        await barrier.release("planning")
        try await creating.value
    }

    @Test("A run that fails after New Task filed it selects the Task with the error; nothing is filed again")
    func newTaskRunFailureSelectsTheFiledTask() async throws {
        let planning = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(planning.utf8))
        let task = try #require(snapshot.tasks.items.first)
        let receipt = """
        {"task": {"id": "\(task.task.id)", "identifier": "\(task.task.identifier)", "title": "\(task.task.name)"},
         "graph": null, "session_id": null}
        """
        let query = RegistryQuery { args, _ in
            if args.starts(with: ["task", "create"]) {
                throw RegistryQueryError("lf task run failed", stdout: receipt)
            }
            if args.starts(with: ["desktop", "plan"]) { return planning }
            throw RegistryQueryError("A failed run opens no conversation")
        }
        let (model, _) = destinationModel(query: query, repoPath: task.task.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        model.navigation.palette = .newTask
        try await model.createTask(request: "fix the flaky retry test", wave: nil, graph: nil)
        #expect(model.navigation.palette == nil)
        #expect(model.selection == .task(id: task.id))
        #expect(model.navigation.taskRunDrafts[task.id]?.error == "lf task run failed")
    }

    @Test("New Task keeps the sheet when nothing was filed")
    func newTaskFailureBeforeFilingKeepsTheSheet() async throws {
        let query = RegistryQuery { args, _ in
            throw RegistryQueryError("not a repository")
        }
        let (model, _) = destinationModel(query: query, repoPath: "/repo")
        model.navigation.palette = .newTask
        await #expect(throws: RegistryQueryError.self) {
            try await model.createTask(request: "fix it", wave: nil, graph: nil)
        }
        #expect(model.navigation.palette == .newTask)
    }

    @Test func dashboardTileOpensConversationInPlace() async throws {
        let planning = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(planning.utf8))
        let task = try #require(snapshot.tasks.items.first)
        let record = try renameFixtureRecord("primary", title: "Task conversation", work: .task(id: #require(task.runtime?.workId)))
        let encoded = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let query = RegistryQuery { args, _ in
            if args.prefix(2) == ["session", "resume"] { return encoded }
            if args.starts(with: ["desktop", "plan"]) { return planning }
            throw RegistryQueryError("Opening in a tile reads nothing else")
        }
        let (model, registry) = destinationModel(query: query, repoPath: task.task.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        model.selectWorkingTasksDashboard()
        let tile = registry.workspace(for: model.workspaceKey(for: .task(id: task.id), repository: "repo"),
                                      title: task.task.name, location: nil)
        try DashboardConversationOffer(model: model, task: task, workspace: tile).inspect()
            .find(viewWithAccessibilityIdentifier: "dashboard-open-conversation-\(task.id)").button().tap()
        for _ in 0..<200 where tile.multiplexer.pane(forSessionId: record.id) == nil {
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(tile.multiplexer.pane(forSessionId: record.id) != nil)
        #expect(tile.conversationError == nil)
        #expect(model.navigation.destination == .workingTasks)
    }

    @Test("Task entry exposes Move here without taking over the external terminal", arguments: [false, true])
    func taskConversationActiveElsewhere(explicitSession: Bool) async throws {
        let data = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let taskID = try #require(task.runtime?.workId)
        var record = try renameFixtureRecord("external-primary", title: "Task conversation",
            work: .task(id: taskID))
        record.workspace = SessionWorkspace(machineId: "machine-local", worktree: "/src/task",
            taskId: taskID, unavailable: nil)
        // Every preparation reports the external client as still active.
        let active = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let query = RegistryQuery { args, _ in
            if args.starts(with: ["desktop", "plan"]) { return data }
            if args.starts(with: ["desktop", "sessions"]) { return #"{"entries":[\#(active)],"next":null}"# }
            if args.prefix(2) == ["session", "resume"] { return active }
            throw RegistryQueryError("Unexpected Task conversation operation")
        }
        let (model, registry) = destinationModel(query: query, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        if explicitSession {
            await model.openTaskLink(TaskLink(issue: task.task.identifier, repo: wave.wave.repo, session: record.id))
        } else {
            await model.openTaskConversation(task.id)
        }
        let workspace = try #require(registry.selectedWork.flatMap(registry.retained))
        let store = registry.sessionStore(repository: "repo", repoPath: wave.wave.repo, query: query)
        #expect(workspace.multiplexer.focusedPane.sessionId == record.id)
        #expect(workspace.conversationError == nil)
        #expect(model.taskOpening?.status == .usable)
        #expect(store.sessions.first?.state == .elsewhere)
        #expect(store.sessions.first?.surface == nil)
        // Inspect the production placeholder directly: ViewInspector cannot resolve
        // Observation environment objects while descending the workspace tree.
        let view = SessionPaneView(pane: workspace.multiplexer.focusedPane, isFocused: true,
            sessions: store, workspace: workspace, model: model, onFocus: { _, _ in })
            ._sessionPlaceholder(try #require(store.sessions.first))
        let move = try view.inspect().find(viewWithAccessibilityIdentifier: "session-move-here-\(record.id)").button()
        #expect(!move.isDisabled())
        #expect(throws: Never.self) {
            try view.inspect().find(text: "Moving stops the other client. Unsent text typed there is lost.")
        }
        try move.tap()
        let deadline = ContinuousClock.now + .seconds(3)
        while store.sessions.first?.state != .prepared, ContinuousClock.now < deadline { await Task.yield() }
        #expect(store.sessions.first?.state == .prepared)
        #expect(store.sessions.first?.surface?.id == record.id)
        #expect(store.sessions.first?.launchArguments == ["lf", "session", "resume", record.id])
        #expect(workspace.multiplexer.focusedPane.sessionId == record.id)
    }

    @Test(arguments: [false, true])
    func openingInstallsItsPreparedPaneWithoutPreparingAgain(unprepared: Bool) async throws {
        let data = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        var record = try renameFixtureRecord("primary", title: "Conversation", work: .task(id: #require(task.runtime?.workId)))
        var wire = try JSONSerialization.jsonObject(with: JSONEncoder().encode(record)) as! [String: Any]
        wire["actions"] = sessionActionFixture(state: "closed")
        record = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: wire))
        record.workspace = try JSONDecoder().decode(SessionWorkspace.self, from: JSONSerialization.data(withJSONObject: [
            "machine_id": "machine-local", "worktree": "/src/task", "task_id": task.runtime?.workId ?? task.id
        ]))
        let encoded = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let barrier = LinkedDestinationBarrier()
        let query = RegistryQuery { args, _ in
            if args.starts(with: ["session", "resume", "--task"]) { await barrier.wait("prepare"); return encoded }
            if args.starts(with: ["desktop", "plan"]) { await barrier.wait("planning"); return data }
            throw RegistryQueryError("Unexpected opening operation")
        }
        let (model, registry) = destinationModel(query: query, repoPath: wave.wave.repo)
        var initial = snapshot
        if unprepared {
            var planning = try JSONSerialization.jsonObject(with: Data(data.utf8)) as! [String: Any]
            var inventory = try #require(planning["tasks"] as? [String: Any])
            var allTasks = try #require(inventory["items"] as? [[String: Any]])
            let index = try #require(allTasks.firstIndex { ($0["task"] as? [String: Any])?["id"] as? String == task.id })
            allTasks[index]["runtime"] = NSNull()
            var reference = allTasks[index]["reference"] as! [String: Any]
            reference["workspace"] = NSNull()
            allTasks[index]["reference"] = reference
            inventory["items"] = allTasks
            planning["tasks"] = inventory
            initial = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: planning))
        }
        model.applyFixture(roadmap: .available(initial), waves: .available([]), workActivity: .loading, repos: [])
        let opening = Task { await model.openTaskConversation(task.id) }
        while !(await barrier.contains("prepare")) { await Task.yield() }
        let workspace = try #require(registry.selectedWork.flatMap(registry.retained))
        #expect(model.selection == .task(id: task.id))
        #expect(workspace.openingConversation)
        let paneID = workspace.multiplexer.focusedPaneId
        await barrier.release("prepare")
        let deadline = ContinuousClock.now + .seconds(3)
        while !(await barrier.contains("planning")), ContinuousClock.now < deadline { await Task.yield() }
        try #require(await barrier.contains("planning"))
        #expect(workspace.openingConversation)
        #expect(workspace.multiplexer.focusedPaneId == paneID)
        let store = registry.sessionStore(repository: "repo", repoPath: wave.wave.repo, query: query)
        await barrier.release("planning")
        await opening.value
        #expect(!workspace.openingConversation)
        #expect(store.sessions.first?.state == .prepared)
        #expect(workspace.multiplexer.focusedPane.sessionId == record.id)
        #expect(model.taskOpening?.status == .opening) // Native mount/input is separate.
    }

    @Test("Open conversation opens whatever the pane names, held or not, even while native mount is pending")
    func openConversationReplacesAPaneTheStoreDoesNotHold() async throws {
        let data = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        var record = try renameFixtureRecord("primary", title: "Task conversation", work: .task(id: #require(task.runtime?.workId)))
        var wire = try JSONSerialization.jsonObject(with: JSONEncoder().encode(record)) as! [String: Any]
        wire["actions"] = sessionActionFixture(state: "closed")
        record = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: wire))
        record.workspace = try JSONDecoder().decode(SessionWorkspace.self, from: JSONSerialization.data(withJSONObject: [
            "machine_id": "machine-local", "worktree": "/src/task", "task_id": task.runtime?.workId ?? task.id
        ]))
        let encoded = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let prepares = DestinationReadCounter()
        let query = RegistryQuery { args, _ in
            if args.contains("connect") { _ = await prepares.next(); return encoded }
            if args.starts(with: ["desktop", "plan"]) { return data }
            throw RegistryQueryError("Unexpected opening operation")
        }
        let (model, registry) = destinationModel(query: query, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        // A saved layout restores a headless Run the window's Session inventory excludes.
        let workspace = registry.workspace(for: model.workspaceKey(for: .task(id: task.id), repository: "repo"),
            title: task.task.name, location: nil)
        workspace.multiplexer.load(sessionId: "headless-run")

        await model.openTaskConversation(task.id)
        #expect(workspace.multiplexer.sessionPane?.sessionId == record.id)
        #expect(model.taskOpening?.status == .opening)

        workspace.multiplexer.load(sessionId: "headless-run")
        await model.openTaskConversation(task.id)
        #expect(workspace.multiplexer.sessionPane?.sessionId == record.id)
        #expect(await prepares.next() == 3)
    }

    @Test func conversationHandedToAnotherRepositoryLeavesNoOpeningHere() async throws {
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(try fixture().utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let query = RegistryQuery { _, _ in throw RegistryQueryError("The receiving window opens the conversation") }
        let (model, _) = destinationModel(query: query, repoPath: "/origin")
        var requests: [(String, LinkedWork?)] = []
        model.openRepository = { requests.append(($0, $1)) }
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])

        await model.openTaskLink(TaskLink(issue: task.task.identifier, repo: wave.wave.repo, conversation: true),
            expectedTaskID: task.id)
        #expect(model.taskOpening == nil)
        #expect(requests.last?.1 == .task(TaskLink(issue: task.task.identifier, repo: wave.wave.repo, conversation: true)))
    }

    @Test(arguments: ["valid", "updated-inventory", "unavailable-inventory", "foreign-session", "missing-task", "unavailable-plan"])
    func ensuredTaskOpeningRetainsOtherSessionsAndValidation(outcome: String) async throws {
        let data = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let primary = try renameFixtureRecord("primary", title: "Primary", work: .task(id:
            outcome == "foreign-session" ? "another-task" : #require(task.runtime?.workId)))
        let retained = try renameFixtureRecord("retained", title: "Other conversation", work: nil)
        let encoded = String(decoding: try JSONEncoder().encode(primary), as: UTF8.self)
        let other = String(decoding: try JSONEncoder().encode(retained), as: UTF8.self)
        let updated = other.replacingOccurrences(of: "Other conversation", with: "Updated conversation")
        let missing = try oneTask(data, taskId: "absent")
        let reads = DestinationReadCounter()
        let barrier = LinkedDestinationBarrier()
        let (model, workspaceRegistry) = destinationModel(query: RegistryQuery { args, _ in
            if args.starts(with: ["desktop", "sessions"]) {
                if await reads.next() == 1 { return #"{"entries":[\#(other)],"next":null}"# }
                if outcome == "updated-inventory" { return #"{"entries":[\#(updated)],"next":null}"# }
            }
            if args.prefix(2) == ["session", "resume"] { return encoded }
            if args.starts(with: ["desktop", "plan"]) {
                if outcome == "updated-inventory" { await barrier.wait("planning") }
                if outcome == "unavailable-plan" { throw RegistryQueryError("Planning disconnected") }
                return outcome == "missing-task" ? missing : data
            }
            throw RegistryQueryError("Opening cannot wait for unrelated Session pages")
        }, repoPath: wave.wave.repo)
        await model.refreshSessions()
        if outcome == "unavailable-inventory" { await model.refreshSessions() }
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        if outcome == "updated-inventory" {
            let opening = Task { await model.openTaskConversation(task.id) }
            while !(await barrier.contains("planning")) { await Task.yield() }
            await model.refreshSessions()
            await barrier.release("planning")
            await opening.value
        } else { await model.openTaskConversation(task.id) }

        #expect(model.sessions.value?.contains { $0.id == retained.id } == true)
        if ["valid", "unavailable-inventory", "updated-inventory"].contains(outcome) {
            #expect(installedSession(model, workspaceRegistry) == primary)
            #expect(model.sessions.value?.contains { $0.id == primary.id } == true)
            // Preparation cannot establish provider input readiness.
            #expect(model.taskOpening?.status != .usable)
            if outcome == "unavailable-inventory" {
                #expect(model.sessions.errorMessage == "Opening cannot wait for unrelated Session pages")
            }
            if outcome == "updated-inventory" {
                #expect(model.sessions.value?.first { $0.id == retained.id }?.title == "Updated conversation")
            }
        } else {
            #expect(model.taskOpening?.status == .failed)
            #expect(model.taskOpening?.reason?.contains(outcome == "foreign-session" ? "was not found in Task"
                : outcome == "missing-task" ? "no longer available" : "Planning disconnected") == true)
        }
    }

    @Test func failedTaskConversationRetainsItsReasonWithoutADuplicateAlert() async throws {
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(try fixture().utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Conversation launcher unavailable") }
        let (model, registry) = destinationModel(query: query, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])

        await model.openTaskConversation(task.id)
        let entry = try #require(registry.retained(model.workspaceKey(for: .task(id: task.id), repository: "repo")))
        #expect(!model.showsTaskLink)
        model.dismissTaskLink()
        model.select(.wave(id: wave.wave.id))
        #expect(entry.conversationError == "Conversation launcher unavailable")
        #expect(!entry.openingConversation)
        #expect(entry.multiplexer.sessionPane?.sessionId == nil)
    }

    @Test(arguments: [false, true])
    func bareTaskOpeningDoesNotAcquireASession(remote: Bool) async throws {
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(try fixture().utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let taskID = task.runtime?.workId ?? task.id
        let remoteIdentity = WorkspaceIdentity(machineId: "peer", worktree: "/peer/retained-checkout")
        let query = RegistryQuery { args, _ in
            if args.starts(with: ["config", "repository"]) {
                return #"{"id":"repository","locators":["repository"]}"#
            }
            if args.starts(with: ["--machine", "peer", "--repository", "repository", "task", "location"]) {
                let index = try #require(args.firstIndex(of: "--request"))
                return String(decoding: try JSONSerialization.data(withJSONObject: [[
                    "request": args[index + 1], "repository_id": "repository", "task_id": taskID,
                    "machine_id": "peer", "observed_at": 1,
                    "location": ["state": "recorded", "task_id": "remote-task", "checkout": remoteIdentity.worktree]
                ]]), as: UTF8.self)
            }
            throw RegistryQueryError("Bare Work opening cannot prepare a checkout or acquire a Session")
        }
        let (model, _) = destinationModel(query: query, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        let link = TaskLink(issue: taskID, repo: wave.wave.repo, session: nil, diff: false,
                            machine: remote ? "peer" : nil, repository: remote ? "repository" : nil)
        await model.openTaskLink(link)
        #expect(model.selection == .task(id: task.id))
        #expect(model.roadmap.value == snapshot)
        #expect(model.navigation.selectedSessionId == nil)
        #expect(model.taskOpening?.status == .opening)
        let page = try #require(model.linkedTaskPage)
        #expect(page.taskID == task.id)
        if remote {
            #expect(model.navigation.preparedTaskWorktrees[task.id] == remoteIdentity)
        } else {
            #expect(model.navigation.preparedTaskWorktrees.isEmpty)
        }
        model.finishTaskPage(page)
        #expect(model.taskOpening?.status == .usable)
    }

    @Test(arguments: ["recorded", "stale", "unavailable", "unrecorded", "wrong-checkout"])
    func remoteOpeningKeepsLocalPlanningAndRetainedDrafts(outcome: String) async throws {
        if outcome == "recorded", ProcessInfo.processInfo.environment["LOOPFLOW_REMOTE_COMPANION_HOST"] == nil {
            let host = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
            try FileManager.default.createDirectory(at: host, withIntermediateDirectories: true)
            defer { try? FileManager.default.removeItem(at: host) }
            let executable = try #require(Bundle.main.executableURL)
            try #require(executable.lastPathComponent == "swiftpm-testing-helper")
            let owned = host.appendingPathComponent(executable.lastPathComponent)
            try FileManager.default.copyItem(at: executable, to: owned)
            let helper = host.appendingPathComponent("lf")
            try "#!/bin/sh\nexit 93\n".write(to: helper, atomically: true, encoding: .utf8)
            try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: helper.path)
            try JSONSerialization.data(withJSONObject: ["lf_path": helper.path])
                .write(to: host.appendingPathComponent("LoopflowDevControl.json"))
            let process = Process()
            process.executableURL = owned
            process.arguments = ["--test-bundle-path", try #require(Bundle(for: RemoteCompanionBundleMarker.self).executablePath),
                "--testing-library", "swift-testing", "--filter", "WorkDestinationTests/remoteOpeningKeepsLocalPlanningAndRetainedDrafts", "--no-parallel"]
            process.environment = ProcessInfo.processInfo.environment.merging(
                ["LOOPFLOW_REMOTE_COMPANION_HOST": host.path], uniquingKeysWith: { _, new in new })
            let output = Pipe()
            process.standardOutput = output
            process.standardError = output
            try process.run()
            let log = output.fileHandleForReading.readDataToEndOfFile()
            process.waitUntilExit()
            print(String(decoding: log, as: UTF8.self))
            try #require(process.terminationStatus == 0)
            return
        }
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(try fixture().utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let taskID = try #require(task.runtime?.workId)
        let localIdentity = try #require(task.reference.workspace?.identity)
        let remoteID = "task_remote_correspondence"
        let remoteIdentity = WorkspaceIdentity(machineId: "peer", worktree: "/peer/retained-checkout")
        var record = try renameFixtureRecord("peer-session", title: "Remote review", work: .task(id: remoteID))
        record.workspace = try JSONDecoder().decode(SessionWorkspace.self, from: JSONSerialization.data(withJSONObject: [
            "machine_id": "peer", "worktree": outcome == "wrong-checkout" ? "/changed" : remoteIdentity.worktree,
            "task_id": remoteID
        ]))
        var surface = try JSONSerialization.jsonObject(with: JSONEncoder().encode(record)) as! [String: Any]
        // Its agent runs on the peer with no terminal showing it.
        surface["state"] = "detached"
        surface["actions"] = sessionActionFixture(state: "detached")
        surface["open_argv"] = ["ssh", "-tt", "peer", "owned-fixture"]
        record = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: surface))
        let encoded = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let query = RegistryQuery { args, cwd in
            if args.starts(with: ["config", "repository"]) { return #"{"id":"selected-plan","locators":["selected-plan","repository"]}"# }
            guard args.starts(with: ["--machine", "peer", "--repository", "repository"]), cwd == nil else {
                throw RegistryQueryError("Remote paths must never become local working directories")
            }
            let command = Array(args.dropFirst(4))
            if command.starts(with: ["task", "location"]) {
                let requestIndex = try #require(command.firstIndex(of: "--request"))
                let request = outcome == "stale" ? "previous-request" : command[requestIndex + 1]
                let location: [String: Any] = outcome == "unavailable" ? ["state": "unavailable", "reason": "Offline"]
                    : outcome == "unrecorded" ? ["state": "unrecorded"]
                    : ["state": "recorded", "task_id": remoteID, "checkout": remoteIdentity.worktree]
                return String(decoding: try JSONSerialization.data(withJSONObject: [[
                    "request": request, "repository_id": "repository", "task_id": taskID,
                    "machine_id": "peer", "observed_at": 1, "location": location
                ]]), as: UTF8.self)
            }
            if command.starts(with: ["desktop", "sessions"]) { return #"{"entries":[\#(encoded)],"next":null}"# }
            if command.prefix(2) == ["session", "resume"] { return encoded }
            throw RegistryQueryError("No checkout, provider launch or transfer permitted")
        }
        let (model, workspaceRegistry) = destinationModel(query: query, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        let registry = SessionsWorkspaceRegistry(localMachineId: localIdentity.machineId)
        let local = fixtureWork(registry, localIdentity)
        local.multiplexer.newShell(command: ["retained-local-shell"], location: local.location)
        let layout = local.multiplexer.layout
        let files = local.files(issue: taskID, cwd: localIdentity.worktree, query: query)
        let document = files.document("draft.txt")
        document.editor.string = "unfinished local draft"
        let link = TaskLink(issue: taskID, repo: wave.wave.repo, session: record.id, diff: true,
                            machine: "peer", repository: "repository")
        await model.openTaskLink(link)
        #expect(model.repoPath == wave.wave.repo)
        #expect(model.roadmap.value == snapshot)
        #expect(local.multiplexer.layout == layout)
        #expect(document.editor.string == "unfinished local draft")
        if outcome == "recorded" {
            let opened = try #require(workspaceRegistry.selectedWork.flatMap(workspaceRegistry.retained))
            let pane = try #require(opened.multiplexer.layout.allPanes.first { $0.sessionId == record.id })
            let request = LinkedSession(generation: 0, record: record, task: task, showsChanges: true, location: pane.executionLocation)
            #expect(request.record.id == record.id)
            #expect(request.location?.machineID == "peer")
            #expect(model.selection == .task(id: task.id))
            #expect(model.navigation.preparedTaskWorktrees[task.id] == remoteIdentity)
            #expect(model.taskOpening?.status != .usable)
            let remote = registry.workspace(for: WorkWorkspaceKey(repository: "repository", kind: .task, id: task.runtime?.workId ?? task.id), title: task.task.name, location: remoteIdentity)
            remote.multiplexer.load(sessionId: record.id, location: record.workspace?.identity)
            let sessionOnly = LinkedSession(generation: request.generation, record: record,
                task: task, showsChanges: false, location: request.location)
            #expect(try registry.openLinkedFiles(sessionOnly, in: remote.key, query: query) == nil)
            let companion = try remote.files(for: .browser(taskId: task.id, tabs: [.files], selected: "files", execution: request.location), task: task, query: query)
            companion.document("remote-draft.txt").editor.string = "retained peer draft"
            #expect(remote.multiplexer.browserPane.map(remote.multiplexer.isVisible) == false)
            #expect(try registry.openLinkedFiles(request, in: remote.key, query: query) === companion)
            #expect(companion.document("remote-draft.txt").editor.string == "retained peer draft")
            let sessions = local.sessionStore(repoPath: wave.wave.repo, query: query)
            sessions.retainLocation(try #require(request.location), for: record)
            sessions.reconcile([record])
            await sessions.select(record.id)
            #expect(sessions.sessions.first?.state == .prepared)
            #expect(sessions.sessions.first?.record.openArgv.first == "ssh")
            #expect(sessions.presentationDirectory(for: record, checkout: remoteIdentity.worktree) == wave.wave.repo)
            #expect(sessions.taskSubject(for: record) == .task(id: taskID))
            #expect(companion.issue == remoteID)
            #expect(companion.query.remoteMachine == "peer")
            #expect(remote.multiplexer.browserPane.map(remote.multiplexer.isVisible) == true)
            let targetPane = remote.multiplexer.focusedPane
            remote.multiplexer.setZoom(targetPane.id, enabled: true)
            let target = DesktopPaneTarget(repository: "repository", window: UUID().uuidString,
                work: remote.key, pane: targetPane.id, incarnation: targetPane.incarnation, tab: nil)
            try DesktopAction.perform(DesktopActionRequest("run", at: target), model: model, workspaces: registry)
            let shell = try #require(remote.multiplexer.layout.allPanes.first { $0.content.selectedShell?.location == remoteIdentity })
            let command = try #require(remote.multiplexer.layout.pane(for: shell.id)?.content.selectedShell?.command)
            #expect(Array(command.dropFirst()) == ["--machine", "peer", "--repository", "repository",
                "task", "shell", remoteID, "--checkout", remoteIdentity.worktree])
            let splitID = try DesktopAction.perform(DesktopActionRequest("run", at: target), model: model, workspaces: registry)
            #expect(remote.multiplexer.shell(splitID.tab ?? "")?.execution == request.location)
            #expect(remote.multiplexer.layout.pane(for: try #require(splitID.pane))?.content.selectedShell?.command == command)
            // A scripted shell tab keeps the caller's focus and zoom.
            #expect(remote.multiplexer.layout.pane(for: targetPane.id) == targetPane)
            #expect(remote.multiplexer.focusedPaneId == targetPane.id)
            #expect(remote.multiplexer.zoomedPaneId == targetPane.id)
            #expect(shell.kind == .shell)
            #expect(companion.document("remote-draft.txt").editor.string == "retained peer draft")
            remote.multiplexer.showBrowser(.files, location: WorkspaceIdentity(machineId: "peer", worktree: "/replacement"))
            let unchanged = remote.multiplexer.layout
            #expect(throws: DesktopTargetError.self) {
                try DesktopAction.perform(DesktopActionRequest("run", at: target), model: model, workspaces: registry)
            }
            #expect(remote.multiplexer.layout == unchanged)

            #expect(local.multiplexer.layout == layout)
        } else {
            #expect(model.taskOpening?.status == .failed)
            #expect(model.navigation.preparedTaskWorktrees[task.id] == nil)
            #expect(registry.keys == [fixtureWorkKey(localIdentity)])
        }
    }

    @Test func resolvingTaskKeepsTheWorkspaceVisible() async throws {
        let data = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let task = try #require(snapshot.waves.first?.tasks.items.first)
        let exact = try oneTask(data, taskId: task.id)
        let barrier = LinkedDestinationBarrier()
        let (model, _) = destinationModel(query: RegistryQuery { _, _ in
            await barrier.wait("read")
            return exact
        })
        let url = TaskLink(issue: task.task.identifier, repo: nil)
        let opening = Task { await model.openTaskLink(url) }
        while !(await barrier.contains("read")) { await Task.yield() }
        #expect(model.taskLinkReading.isLoading)
        #expect(!model.showsTaskLink)
        await barrier.release("read")
        await opening.value
        #expect(model.selection == .task(id: task.id))
        #expect(!model.showsTaskLink)
    }

    @Test(arguments: [false, true])
    func reopeningLoadedTaskPreservesItsSessionWithoutAnotherRead(runtimeSelection: Bool) async throws {
        let data = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let (model, _) = destinationModel(query: RegistryQuery { _, _ in
            throw RegistryQueryError("A loaded destination needs no new read")
        }, repoPath: wave.wave.repo)
        // The ordinary outline has already published this planning inventory.
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]),
                           workActivity: .loading, repos: [])
        model.openTaskDestination(wave: wave, task: task)
        let selectedID = runtimeSelection ? try #require(task.runtime?.workId) : task.id
        model.select(.task(id: selectedID))
        model.navigation.selectedSessionId = "retained-conversation"
        model.navigation.content = .terminals
        let url = TaskLink(issue: task.task.identifier, repo: wave.wave.repo)
        await model.openTaskLink(url)
        #expect(model.selection == .task(id: selectedID))
        #expect(model.navigation.selectedSessionId == "retained-conversation")
        #expect(model.navigation.content == .terminals)
        #expect(model.taskLinkReading.errorMessage == nil)
        #expect(!model.showsTaskLink)

        // Reopening what is already open changes nothing, so nothing redraws.
        let invalidations = OSAllocatedUnfairLock(initialState: 0)
        withObservationTracking {
            _ = (model.repoPath, model.taskLink, model.showsTaskLink,
                 model.navigation.selectedTaskEvidence?.task, model.navigation.recentDestinations)
        } onChange: {
            invalidations.withLock { $0 += 1 }
        }
        await model.openTaskLink(url)
        #expect(invalidations.withLock { $0 } == 0)
        #expect(model.navigation.selectedSessionId == "retained-conversation")
    }

    @Test func refreshedPlanningDoesNotReuseAnOldLinkResult() async throws {
        let data = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let exact = try oneTask(data, taskId: task.id)
        let empty = try oneTask(data, taskId: "absent")
        let removed = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(empty.utf8))
        let reads = DestinationReadCounter()
        let (model, _) = destinationModel(query: RegistryQuery { _, _ in
            await reads.next() == 1 ? exact : empty
        }, repoPath: wave.wave.repo)
        let url = TaskLink(issue: task.task.identifier, repo: wave.wave.repo)
        await model.openTaskLink(url)
        model.applyFixture(roadmap: .available(removed), waves: .available([]),
                           workActivity: .loading, repos: [])
        await model.openTaskLink(url)
        #expect(model.showsTaskLink)
        #expect(model.taskLinkReading.value?.waves.flatMap { $0.tasks.items }.isEmpty == true)
    }

    @Test func taskLinkRetainsLaterPagesAndReusesTheConversation() async throws {
        let data = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let record = try renameFixtureRecord("first-page", title: "Review", work: .task(id: #require(task.runtime?.workId)))
        let other = try renameFixtureRecord("later-page", title: "Another conversation", work: .task(id: #require(task.runtime?.workId)))
        let encoded = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let otherEncoded = String(decoding: try JSONEncoder().encode(other), as: UTF8.self)
        let (model, workspaceRegistry) = destinationModel(query: RegistryQuery { args, _ in
            guard args.prefix(2) == ["desktop", "sessions"] else {
                throw RegistryQueryError("Planning is already available")
            }
            if args.contains("--after") { return #"{"entries":[\#(otherEncoded)],"next":null}"# }
            return #"{"entries":[\#(encoded)],"next":"remaining-history"}"#
        }, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]),
                           workActivity: .loading, repos: [])
        let url = TaskLink(issue: task.task.identifier, repo: wave.wave.repo, session: record.id)
        await model.openTaskLink(url)
        #expect(installedSession(model, workspaceRegistry)?.id == record.id)
        #expect(model.navigation.selectedSessionId == record.id)
        #expect(model.taskLinkReading.errorMessage == nil)
        // Reopening from retained evidence must preserve the conversation too.
        await model.openTaskLink(url)
        #expect(model.navigation.selectedSessionId == record.id)
        #expect(model.sessions.value?.map(\.id) == [record.id, other.id])
    }

    @Test func composedOpeningRetainsTheExactSessionAndChangesPane() async throws {
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(try fixture().utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        var record = try renameFixtureRecord("composed-session", title: "Review", work: .task(id: #require(task.runtime?.workId)))
        let identity = try #require(task.reference.workspace?.identity)
        record.workspace = try JSONDecoder().decode(SessionWorkspace.self, from: JSONSerialization.data(withJSONObject: [
            "machine_id": identity.machineId, "worktree": identity.worktree,
            "task_id": try #require(task.runtime?.workId)
        ]))
        let encoded = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let query = RegistryQuery { args, _ in
            guard args.starts(with: ["desktop", "sessions"]) else { throw RegistryQueryError("No launch or planning mutation permitted") }
            return #"{"entries":[\#(encoded)],"next":null}"#
        }
        let model = WorkModel(query: query, repoPath: wave.wave.repo)
        let workspaceRegistry = SessionsWorkspaceRegistry(localMachineId: identity.machineId)
        model.bindWorkspaces(workspaceRegistry, repository: "repo")
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        let sessionOnly = TaskLink(issue: task.task.identifier, repo: wave.wave.repo, session: record.id)
        await model.openTaskLink(sessionOnly)
        #expect(installedSession(model, workspaceRegistry)?.id == record.id)
        // Adding a companion reuses the installed terminal and its pane.
        let link = TaskLink(issue: try #require(task.runtime?.workId), repo: wave.wave.repo, session: record.id, diff: true)
        await model.openTaskLink(link)
        #expect(installedSession(model, workspaceRegistry)?.id == record.id)
        #expect(model.taskOpening?.status != .usable) // placement is not native readiness
        let workspace = try #require(workspaceRegistry.selectedWork.flatMap(workspaceRegistry.retained))
        let terminal = try #require(workspace.multiplexer.layout.allPanes.first { $0.sessionId == record.id })
        let files = workspace.files(issue: task.task.identifier, cwd: identity.worktree, query: query)
        files.selection = "retained-draft.rs"
        await model.openTaskLink(link)
        #expect(workspace.multiplexer.layout.allPanes.count == 3)
        #expect(workspace.multiplexer.layout.pane(for: terminal.id) == terminal)
        #expect(workspace.multiplexer.focusedPane.content.selectedBrowserTab == .files)
        #expect(workspace.multiplexer.focusedPane.content.location == identity)
        #expect(files.selection == "retained-draft.rs")
        #expect(files.showsChanges)
        let failure = TaskLink(issue: task.task.identifier, repo: wave.wave.repo, session: "missing", diff: true)
        await model.openTaskLink(failure)
        #expect(model.taskOpening?.status == .failed)
        #expect(model.taskOpening?.url == failure.address)
        #expect(model.navigation.selectedSessionId == record.id)
        // Retrying through the chooser uses the same failure receipt as a direct link.
        await model.chooseLinkedTask(wave: wave, task: task)
        #expect(model.taskOpening?.status == .failed)
        #expect(model.taskOpening?.reason == model.taskLinkReading.errorMessage)
    }

    @Test(arguments: ["usable", "connection", "surface", "comparison", "closed", "hidden", "zoomed", "replaced"])
    func composedOpeningWaitsForNativeSessionAndComparison(outcome: String) async throws {
        let (model, registry, record, url, query) = try openingFixture(outcome: outcome)
        await model.openTaskLink(url)
        let workspace = try #require(registry.selectedWork.flatMap(registry.retained))
        let store = registry.sessionStore(repository: "repo", repoPath: try #require(model.repoPath), query: query)
        let task = try #require(model.selection.flatMap { model.task(id: $0.id)?.task })
        let files = workspace.files(issue: task.task.identifier,
            cwd: try #require(record.workspace?.worktree), query: query)
        files.selection = "retained-draft.rs"
        if outcome != "connection", outcome != "comparison" {
            #expect(store.sessions.first?.state == .prepared)
            #expect(model.taskOpening?.status == .opening)
            let pane = workspace.multiplexer.focusedPane
            switch outcome {
            case "closed": workspace.multiplexer.close(pane.id)
            case "hidden": workspace.multiplexer.setCollapsed(paneId: pane.id, collapsed: true)
            case "zoomed": workspace.multiplexer.setZoom(pane.id, enabled: true)
            case "replaced":
                if pane.kind == .session { workspace.multiplexer.replace(pane.id, with: .session(id: "replacement")) }
                else { workspace.multiplexer.showBrowser(.graphLog) }
            case "surface": store.recordPaneFailure(record.id, reason: "Native surface unavailable")
            default: store.recordPaneLive(record.id)
            }
        }
        try await waitForOpening(model)
        let reasons = ["connection": "Connection unavailable", "surface": "Native surface unavailable",
                       "comparison": "Comparison unavailable"]
        #expect(model.taskOpening?.status == (outcome == "usable" ? .usable : .failed))
        if outcome != "usable" {
            #expect(model.taskOpening?.reason == (reasons[outcome] ?? "Opening panes were hidden, closed or replaced."))
        }
        #expect(model.taskOpening?.url == url.address)
        #expect(files.selection == "retained-draft.rs")
    }

    @Test func unavailableWorkspaceMachineDoesNotConnect() async throws {
        let (model, _, _, url, query) = try openingFixture(outcome: "usable")
        let registry = SessionsWorkspaceRegistry()
        model.bindWorkspaces(registry, repository: "repo")
        await model.openTaskLink(url)
        #expect(model.taskOpening?.status == .failed)
        #expect(model.taskOpening?.reason?.contains("Machine is unavailable") == true)
        let store = registry.sessionStore(repository: "repo", repoPath: try #require(model.repoPath), query: query)
        #expect(store.sessions.isEmpty)
    }

    @Test func canceledComparisonCannotSettleNewOpening() async throws {
        let barrier = LinkedDestinationBarrier()
        let (model, registry, record, url, query) = try openingFixture(outcome: "usable", comparisonBarrier: barrier)
        let opening = Task { await model.openTaskLink(url) }
        while !(await barrier.contains("comparison")) { await Task.yield() }
        let workspace = try #require(registry.selectedWork.flatMap(registry.retained))
        let originalPane = try #require(workspace.multiplexer.pane(forSessionId: record.id))
        #expect(workspace.openingConversation)
        let store = registry.sessionStore(repository: "repo", repoPath: try #require(model.repoPath), query: query)
        // New request to the same Session, without its still-reading companion.
        let task = try #require(model.selection.flatMap { model.task(id: $0.id)?.task })
        let next = TaskLink(issue: task.task.identifier, repo: model.repoPath, session: record.id)
        await model.openTaskLink(next)
        #expect(!workspace.openingConversation)
        await barrier.release("comparison")
        await opening.value
        #expect(model.taskOpening?.url == next.address)
        #expect(model.taskOpening?.status == .opening)
        #expect(workspace.multiplexer.focusedPane == originalPane)
        store.recordPaneLive(record.id)
        try await waitForOpening(model)
        #expect(model.taskOpening?.status == .usable)
    }

    @Test func canceledOpeningCannotBecomeUsableFromALateSurface() async throws {
        let (model, registry, record, url, query) = try openingFixture(outcome: "usable")
        await model.openTaskLink(url)
        model.dismissTaskLink()
        let before = model.taskOpening
        let store = registry.sessionStore(repository: "repo", repoPath: try #require(model.repoPath), query: query)
        store.recordPaneLive(record.id)
        await Task.yield()
        #expect(model.taskOpening == before)
        #expect(model.taskOpening?.status == .failed)
    }

    private func waitForOpening(_ model: WorkModel) async throws {
        let deadline = ContinuousClock.now + .seconds(3)
        while model.taskOpening?.status == .opening {
            guard ContinuousClock.now < deadline else { throw RegistryQueryError("Opening never settled") }
            try await Task.sleep(for: .milliseconds(5))
        }
    }

    private func openingFixture(outcome: String, comparisonBarrier: LinkedDestinationBarrier? = nil) throws -> (WorkModel, SessionsWorkspaceRegistry, SessionRecord, TaskLink, RegistryQuery) {
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(try fixture().utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        var record = try renameFixtureRecord("ready-session", title: "Review", work: .task(id: #require(task.runtime?.workId)))
        let identity = try #require(task.reference.workspace?.identity)
        record.workspace = try JSONDecoder().decode(SessionWorkspace.self, from: JSONSerialization.data(withJSONObject: [
            "machine_id": identity.machineId, "worktree": identity.worktree, "task_id": try #require(task.runtime?.workId)
        ]))
        var sessionJSON = try JSONSerialization.jsonObject(with: JSONEncoder().encode(record)) as! [String: Any]
        sessionJSON["state"] = "detached"
        sessionJSON["actions"] = sessionActionFixture(state: "detached")
        record = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: sessionJSON))
        let encoded = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let fixtureURL = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto/task_files.json")
        let root = try JSONSerialization.jsonObject(with: Data(contentsOf: fixtureURL)) as! [String: Any]
        let changes = String(decoding: try JSONSerialization.data(withJSONObject: #require(root["changes"])), as: UTF8.self)
        let query = RegistryQuery { args, _ in
            if args.starts(with: ["desktop", "sessions"]) { return #"{"entries":[\#(encoded)],"next":null}"# }
            if args.prefix(2) == ["session", "resume"] {
                if outcome == "connection" { throw RegistryQueryError("Connection unavailable") }
                return encoded
            }
            if args.starts(with: ["task", "diff"]) {
                if let comparisonBarrier { await comparisonBarrier.wait("comparison") }
                if outcome == "comparison" { throw RegistryQueryError("Comparison unavailable") }
                return changes
            }
            throw RegistryQueryError("Unexpected fixture operation")
        }
        let model = WorkModel(query: query, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        let registry = SessionsWorkspaceRegistry(localMachineId: identity.machineId)
        model.bindWorkspaces(registry, repository: "repo")
        return (model, registry, record, TaskLink(issue: task.task.identifier, repo: wave.wave.repo, session: record.id, diff: true), query)
    }

    @Test(arguments: [false, true]) func delayedPrimarySessionPreparationDoesNotOpenChangesInAnotherTask(conversation: Bool) async throws {
        let data = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let record = try renameFixtureRecord("prepared-session", title: "Review", work: .task(id: #require(task.runtime?.workId)))
        let encoded = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let barrier = LinkedDestinationBarrier()
        let (model, _) = destinationModel(query: RegistryQuery { args, _ in
            guard args.starts(with: ["session", "resume", "--task"]) else { throw RegistryQueryError("Canceled preparation must not read or open another Task") }
            await barrier.wait("ensure")
            return encoded
        }, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        let url = TaskLink(issue: task.task.identifier, repo: wave.wave.repo, diff: true)
        let opening = Task {
            if conversation { await model.openTaskConversation(task.id) }
            else { await model.openTaskLink(url) }
        }
        while !(await barrier.contains("ensure")) { await Task.yield() }
        model.select(.wave(id: wave.wave.id))
        await barrier.release("ensure")
        await opening.value
        #expect(model.selection == .wave(id: wave.wave.id))
    }

    @Test func paletteBindingTargetsOnlyTheSelectedUnassignedSession() async throws {
        let scopes: [WorkReference?] = [nil, .wave(id: "wave-product"), .task(id: "task-bound")]
        for work in scopes {
            let record = try renameFixtureRecord("binding-session", title: "Conversation", work: work)
            let sessions = String(decoding: try JSONEncoder().encode([record]), as: UTF8.self)
            let (model, _) = destinationModel(query: RegistryQuery { args, _ in
                if args.prefix(2) == ["desktop", "sessions"] { return #"{"entries":\#(sessions),"next":null}"# }
                throw RegistryQueryError("No other read or mutation permitted")
            }, repoPath: "/src/loopflow")
            await model.refreshSessions()
            #expect(model.searchDestinations("Bind").isEmpty)
            model.navigation.selectedSessionId = record.id
            #expect(model.searchDestinations("Bind").map(\.id) == (work?.kind == .task ? [] : [.bind(record.id)]))
            model.navigation.selectedSessionId = "missing-session"
            #expect(model.searchDestinations("Bind").isEmpty)
        }
    }

    @Test func taskLinkOpensExactSessionFromLaterPage() async throws {
        let data = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let exact = try oneTask(data, taskId: task.id)
        let record = try renameFixtureRecord("linked-session", title: "Design review", work: .task(id: #require(task.runtime?.workId)))
        let encoded = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let unrelated = try renameFixtureRecord("unrelated", title: "Other work", work: .task(id: "other-task"))
        let otherEncoded = String(decoding: try JSONEncoder().encode(unrelated), as: UTF8.self)
        let (model, workspaceRegistry) = destinationModel(query: RegistryQuery { args, cwd in
            if args.contains("--task") { return exact }
            if args.prefix(2) == ["desktop", "sessions"], cwd == wave.wave.repo {
                if args.contains("--after") { return #"{"entries":[\#(encoded),\#(otherEncoded)],"next":null}"# }
                return #"{"entries":[],"next":"page-two"}"#
            }
            throw RegistryQueryError("No mutation permitted")
        })
        let url = TaskLink(issue: task.task.identifier, repo: wave.wave.repo, session: record.id)
        await model.openTaskLink(url)
        #expect(model.navigation.selectedSessionId == record.id)
        #expect(model.navigation.content == .terminals)
        #expect(installedSession(model, workspaceRegistry)?.id == record.id)
        #expect(!model.showsTaskLink)

        for session in ["missing", unrelated.id] {
            await model.openTaskLink(TaskLink(issue: task.task.identifier, repo: nil, session: session))
            #expect(model.navigation.selectedSessionId == record.id)
            #expect(model.taskLinkReading.errorMessage?.contains(session) == true)
            #expect(model.showsTaskLink)
        }
    }

    @Test func historicalTaskSurvivesCurrentPlanRefresh() async throws {
        let data = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        // Exercise the production URL handler with one exact shared-reader result.
        let exact = try oneTask(data, taskId: task.id)
        let exactQuery = RegistryQuery { args, _ in
            if args.contains("--task") { return exact }
            if args.prefix(2) == ["desktop", "plan"] { return #"{"generated_at":"2026-09-26T00:00:00Z","waves":[],"tasks":{"state":"ok","items":[],"truncated":false}}"# }
            if args.prefix(2) == ["desktop", "sessions"] { return #"{"entries":[],"next":null}"# }
            if args.first == "wave" { return "[]" }
            throw RegistryQueryError("No mutation permitted")
        }
        let destination = WorkModel(query: exactQuery)
        await destination.openTaskLink(TaskLink(issue: task.task.identifier, repo: nil))
        #expect(destination.selection == .task(id: task.id))
        #expect(destination.navigation.content == .details)
        #expect(destination.navigation.selectedSessionId == nil)
        await destination.refresh()
        #expect(destination.selection == .task(id: task.id))
        #expect(destination.task(id: task.id)?.task.task.name == task.task.name)
        #expect(destination.visibleRoadmaps.isEmpty)
        #expect(destination.breadcrumb?.task?.task.id == task.id)
        let registry = SessionsWorkspaceRegistry()
        let workspace = fixtureWork(registry, try #require(task.reference.workspace?.identity))
        workspace.multiplexer.newShell(command: ["local-server"], location: workspace.location)
        let layout = workspace.multiplexer.layout
        let document = workspace.files(issue: task.task.identifier, cwd: wave.wave.repo,
                                       query: exactQuery).document("note.txt")
        document.editor.string = "unsaved draft"
        let retained = SessionsContentView(
            model: destination, repoPath: wave.wave.repo,
            workspaces: registry, machineId: "local", repository: "fixture-repository", query: exactQuery
        )
        #expect(throws: Never.self) {
            try retained.inspect().find(viewWithAccessibilityIdentifier: "sessions-surface")
        }
        destination.bindWorkspaces(registry, repository: "fixture-repository")
        let selectedKey = try #require(registry.selectedWork)
        #expect(registry.retained(selectedKey)?.location == task.reference.workspace?.identity)
        #expect(workspace.multiplexer.layout == layout)
        #expect(document.editor.string == "unsaved draft")
        #expect(throws: Never.self) {
            try WorkSurfaceView(model: destination).inspect().find(viewWithAccessibilityIdentifier: "loopflow-detail-task")
        }
    }

    @Test func historicalRecentSurvivesLeavingItsPageAndRefresh() async throws {
        let data = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let record = try renameFixtureRecord("historical-primary", title: "Retained Task", work: .task(id: #require(task.runtime?.workId)))
        let encoded = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let exact = try oneTask(data, taskId: task.id)
        // Keep the Wave, but remove the historical Task from the current plan.
        let current = try oneTask(data, taskId: "absent")
        let (model, _) = destinationModel(query: RegistryQuery { args, _ in
            if args.prefix(2) == ["session", "resume"] { return encoded }
            if args.contains("--task") { return exact }
            if args.prefix(2) == ["desktop", "plan"] { return current }
            if args.prefix(2) == ["desktop", "sessions"] { return #"{"entries":[\#(encoded)],"next":null}"# }
            if args.first == "wave" { return "[]" }
            throw RegistryQueryError("No mutation permitted")
        }, repoPath: wave.wave.repo)
        await model.openTaskLink(TaskLink(issue: task.task.identifier, repo: nil))
        model.select(.wave(id: wave.wave.id))
        await model.refresh()
        #expect(model.navigation.selectedTaskEvidence == nil)
        #expect(model.searchDestinations("").first?.id == .task(task.id))
        #expect(model.visibleRoadmaps.flatMap { $0.tasks.items }.isEmpty)
        model.setRepoPath("/another-repository")
        #expect(!model.paletteRows.contains { $0.id == .task(task.id) })
        model.setRepoPath(wave.wave.repo)
        #expect(model.searchDestinations("").first?.id == .task(task.id))
        await model.openPaletteTask(task.id)
        #expect(model.selection == .task(id: task.id))
        #expect(model.task(id: task.id)?.task.task.name == task.task.name)
        #expect(model.visibleRoadmaps.flatMap { $0.tasks.items }.isEmpty)
    }

    @Test func recentTaskReadbackPreservesIdentityAndWorkspace() async throws {
        let data = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let current = try oneTask(data, taskId: "absent")
        // Successful lookup of another ID must not redirect a saved destination.
        for response in ["failure", data, current] {
            let (model, _) = destinationModel(query: RegistryQuery { args, _ in
                if args.contains("--task") {
                    if response == "failure" { throw RegistryQueryError("Machine unavailable") }
                    return response
                }
                if args.prefix(2) == ["desktop", "plan"] { return current }
                if args.prefix(2) == ["desktop", "sessions"] { return #"{"entries":[],"next":null}"# }
                if args.first == "wave" { return "[]" }
                throw RegistryQueryError("No mutation permitted")
            }, repoPath: wave.wave.repo)
            await model.refresh()
            model.openTaskDestination(wave: wave, task: task)
            model.select(.wave(id: wave.wave.id))
            await model.refresh()
            await model.openPaletteTask(task.id)
            #expect(model.selection == .wave(id: wave.wave.id))
            #expect(model.navigation.selectedTaskEvidence == nil)
            #expect(model.showsTaskLink)
            if response == current {
                #expect(!model.paletteRows.contains { $0.id == .task(task.id) })
            } else {
                #expect(model.taskLinkReading.errorMessage != nil)
                #expect(model.paletteRows.contains { $0.id == .task(task.id) })
                await model.retryTaskLink()
                #expect(model.selection == .wave(id: wave.wave.id))
                #expect(model.taskLinkReading.errorMessage != nil)
            }
        }
    }

    @Test func paletteSearchIncludesUnstartedTasksAndRanksExactIDs() async throws {
        let data = try fixture()
        let records = try (0..<30).map { try renameFixtureRecord("\($0)", title: "Session \($0)") }
        let sessions = String(decoding: try JSONEncoder().encode(records), as: UTF8.self)
        let (model, _) = destinationModel(query: RegistryQuery { args, _ in
            if args.prefix(2) == ["desktop", "plan"] { return data }
            if args.prefix(2) == ["desktop", "sessions"] { return #"{"entries":\#(sessions),"next":null}"# }
            if args.first == "wave" { return "[]" }
            throw RegistryQueryError("Unavailable")
        }, repoPath: "/src/loopflow")
        await model.refresh()
        let tasks = model.visibleRoadmaps.flatMap { $0.tasks.items }
        let task = try #require(tasks.first)
        #expect(model.searchDestinations(task.task.identifier).first?.id == .task(task.id))
        #expect(model.paletteRows.filter { if case .task = $0.id { true } else { false } }.count == tasks.count)
        #expect(model.searchDestinations(" session 1 ").isEmpty)
        model.remember(.task(task.id))
        #expect(model.searchDestinations("").first?.id == .task(task.id))

    }

    @Test func historicalTaskWithoutPlanningHasNoDeadEndGraphAction() async throws {
        var snapshot = try #require(JSONSerialization.jsonObject(with: Data(fixture().utf8)) as? [String: Any])
        var waves = try #require(snapshot["waves"] as? [[String: Any]])
        var unavailable = try #require(waves[0]["unavailable_tasks"] as? [[String: Any]])
        unavailable[0]["task_id"] = "issue-available"
        waves[0]["unavailable_tasks"] = unavailable
        snapshot["waves"] = waves
        let data = String(decoding: try JSONSerialization.data(withJSONObject: snapshot), as: UTF8.self)
        let exact = try oneTask(data, taskId: "issue-available")
        let (model, _) = destinationModel(query: RegistryQuery { _, _ in exact })
        await model.openTaskLink(TaskLink(issue: "PRD-52", repo: nil))
        #expect(model.selection == .task(id: "issue-available"))
        #expect(model.paletteRows.contains { $0.id == .graphLog("issue-available") })
    }

    @Test func ambiguousAndUnavailableLinksPreserveTheWorkspace() async throws {
        let data = try fixture()
        for response in [data, #"{"generated_at":"2026-09-26T00:00:00Z","waves":[],"tasks":{"state":"ok","items":[],"truncated":false}}"#, "transport-error"] {
            let (model, _) = destinationModel(query: RegistryQuery { args, _ in
                if args.contains("--task") {
                    if response == "transport-error" { throw RegistryQueryError("Machine unavailable") }
                    return response
                }
                if args.prefix(2) == ["desktop", "plan"] { return data }
                if args.prefix(2) == ["desktop", "sessions"] { return #"{"entries":[],"next":null}"# }
                if args.first == "wave" { return "[]" }
                throw RegistryQueryError("No mutation permitted")
            }, repoPath: "/src/loopflow")
            await model.refresh()
            let wave = try #require(model.visibleRoadmaps.first)
            model.select(.wave(id: wave.wave.id))
            await model.openTaskLink(TaskLink(issue: "LOO-303", repo: nil))
            #expect(model.selection == .wave(id: wave.wave.id))
            #expect(model.showsTaskLink)
            if response == "transport-error" {
                #expect(model.taskLinkReading.errorMessage == "Machine unavailable")
            } else {
                #expect(model.taskLinkReading.errorMessage == nil)
            }
        }
    }

    @Test func inspectionForwardsTaskControlIndependentlyOfSessionFailure() throws {
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(fixture().utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let (model, _) = destinationModel(query: RegistryQuery { _, _ in
            throw RegistryQueryError("Inspection must not read or mutate Work")
        }, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        model.openTaskDestination(wave: wave, task: task)
        model.navigation.selectedSessionId = "not-read-yet"
        let workspaces = SessionsWorkspaceRegistry()
        let report = model.inspectDesktop(repository: "plan", window: UUID(), workspaces: workspaces)
        #expect(report.reading == "loading")
        #expect(report.actions.map(\.name).contains("back"))
        #expect(report.actions.map(\.name).contains("send"))
        #expect(report.task?.reading == "current")
        #expect(report.task?.actions == task.actions)
        #expect(report.task?.runControl == task.runControl)
        #expect(report.task?.roadmapGeneratedAt == snapshot.generatedAt)
        #expect(report.task?.conditionObservedAt == task.condition.observedAt)
        #expect(report.session?.reading == "loading")
        #expect(report.session?.actions == nil)
        #expect(report.session?.observedAt == nil)
        #expect(workspaces.inspect().isEmpty)
    }

    @Test func inspectionUsesTheRepositoryInventoryForBothTaskIdentities() throws {
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(fixture().utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let runtimeID = try #require(task.runtime?.workId)
        let (model, _) = destinationModel(query: RegistryQuery { _, _ in
            throw RegistryQueryError("Inspection must not read Work")
        }, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        let registry = SessionsWorkspaceRegistry()
        for id in [task.id, runtimeID] {
            model.select(.task(id: id))
            #expect(model.task(id: id)?.task.id == task.id)
            let reading = model.inspectDesktop(repository: "plan", window: UUID(), workspaces: registry)
            #expect(reading.task?.reading == "current")
            #expect(reading.task?.actions == task.actions)
        }
        // A repository window cannot offer actions from another repo's inventory.
        model.setRepoPath("/another-repository")
        model.navigation.selection = .task(id: task.id)
        let outside = model.inspectDesktop(repository: "other-plan", window: UUID(), workspaces: registry)
        #expect(outside.task?.reading == "unavailable")
        #expect(outside.task?.actions == nil)
        #expect(outside.task?.conditionObservedAt == nil)
    }

    @Test func inspectionKeepsStaleDatesButNeverHistoricalActions() throws {
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(fixture().utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let (model, _) = destinationModel(query: RegistryQuery { _, _ in
            throw RegistryQueryError("Inspection must not read or mutate Work")
        }, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        model.openTaskDestination(wave: wave, task: task)
        model.applyFixture(roadmap: .unavailable(lastGood: snapshot, reason: "Disconnected"),
                           waves: .available([]), workActivity: .loading, repos: [])
        let stale = model.inspectDesktop(repository: "plan", window: UUID(), workspaces: SessionsWorkspaceRegistry())
        #expect(stale.task?.reading == "unavailable")
        #expect(stale.task?.reason == "Disconnected")
        #expect(stale.task?.roadmapGeneratedAt == snapshot.generatedAt)
        #expect(stale.task?.actions == nil)
        #expect(stale.task?.runControl == nil)

        let empty = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(
            #"{"generated_at":"2026-10-08T21:00:00Z","waves":[],"tasks":{"state":"ok","items":[],"truncated":false}}"#.utf8))
        model.applyFixture(roadmap: .available(empty), waves: .available([]), workActivity: .loading, repos: [])
        model.select(.task(id: task.id))
        let removed = model.inspectDesktop(repository: "plan", window: UUID(), workspaces: SessionsWorkspaceRegistry())
        #expect(removed.task?.reading == "unavailable")
        #expect(removed.task?.actions == nil)
        #expect(removed.task?.conditionObservedAt == nil)
        #expect(removed.task?.roadmapGeneratedAt == empty.generatedAt)
    }

    @Test func inspectionForwardsSessionActionsThenWithdrawsThemOnReadFailure() async throws {
        let record = try renameFixtureRecord("selected-session", title: "Review", work: nil)
        let encoded = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let reads = DestinationReadCounter()
        let (model, _) = destinationModel(query: RegistryQuery { args, _ in
            guard args.prefix(2) == ["desktop", "sessions"] else { throw RegistryQueryError("No other reads permitted") }
            if await reads.next() > 1 { throw RegistryQueryError("Session source disconnected") }
            return #"{"entries":[\#(encoded)],"next":null}"#
        }, repoPath: "/src/loopflow")
        await model.refreshSessions()
        model.navigation.selectedSessionId = record.id
        let registry = SessionsWorkspaceRegistry()
        let first = model.inspectDesktop(repository: "plan", window: UUID(), workspaces: registry)
        #expect(first.session?.reading == "current")
        #expect(first.session?.actions == record.actions)
        #expect(first.session?.machineId == record.workspace?.machineId)
        #expect(first.task == nil)
        await model.refreshSessions()
        let failed = model.inspectDesktop(repository: "plan", window: UUID(), workspaces: registry)
        #expect(failed.session?.id == record.id)
        #expect(failed.session?.reading == "unavailable")
        #expect(failed.session?.reason == "Session source disconnected")
        #expect(failed.session?.actions == nil)
        #expect(model.navigation.selectedSessionId == record.id)
        #expect(registry.inspect().isEmpty)
    }

    @Test func inspectionDoesNotOfferActionsFromAnIncompleteSessionEnumeration() async throws {
        let record = try renameFixtureRecord("retained-session", title: "Draft", work: nil)
        let encoded = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let reads = DestinationReadCounter()
        let barrier = LinkedDestinationBarrier()
        let (model, _) = destinationModel(query: RegistryQuery { args, _ in
            guard args.prefix(2) == ["desktop", "sessions"] else { throw RegistryQueryError("No other reads permitted") }
            if args.contains("--after") {
                await barrier.wait("last-page")
                return #"{"entries":[],"next":null}"#
            }
            if await reads.next() == 1 { return #"{"entries":[\#(encoded)],"next":null}"# }
            return #"{"entries":[],"next":"last-page"}"#
        }, repoPath: "/src/loopflow")
        await model.refreshSessions()
        model.navigation.selectedSessionId = record.id
        let refreshing = Task { await model.refreshSessions() }
        while !(await barrier.contains("last-page")) { await Task.yield() }
        let partial = model.inspectDesktop(repository: "plan", window: UUID(), workspaces: SessionsWorkspaceRegistry())
        #expect(partial.session?.reading == "updating")
        #expect(partial.session?.actions == nil)
        #expect(model.sessions.value?.contains { $0.id == record.id } == true)
        await barrier.release("last-page")
        await refreshing.value
        let removed = model.inspectDesktop(repository: "plan", window: UUID(), workspaces: SessionsWorkspaceRegistry())
        #expect(removed.session?.reading == "unavailable")
        #expect(removed.session?.actions == nil)
        #expect(model.navigation.selectedSessionId == record.id)
    }

    @Test(arguments: ["usable", "failed", "canceled"])
    func plainTaskPageWaitsForItsOwnContent(outcome: String) async throws {
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(try fixture().utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let (model, _) = destinationModel(query: RegistryQuery { _, _ in
            throw RegistryQueryError("No Session preparation is required to show a Task page")
        }, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        let url = TaskLink(issue: task.task.identifier, repo: wave.wave.repo)
        await model.openTaskLink(url)
        let old = try #require(model.linkedTaskPage)
        #expect(model.taskOpening?.status == .opening)
        await model.openTaskLink(url)
        let current = try #require(model.linkedTaskPage)
        #expect(old != current)
        model.finishTaskPage(old)
        model.finishTaskPage(old, error: "Old failure")
        #expect(model.taskOpening?.status == .opening)
        if outcome == "canceled" { model.select(.wave(id: wave.wave.id)) }
        model.finishTaskPage(current, error: outcome == "failed" ? "Machine unavailable" : nil)
        #expect(model.taskOpening?.status == (outcome == "usable" ? .usable : .failed))
        if outcome == "failed" { #expect(model.taskOpening?.reason == "Machine unavailable") }
        #expect(model.linkedTaskPage == nil)
    }

    @Test func repositoryLookupFailureIsInspectableWithoutAWindow() async throws {
        let router = WorkLinkRouter()
        let missing = "/nonexistent-loopflow-fixture-\(UUID().uuidString)"
        do {
            _ = try await router.openRepository(path: missing, link: nil, query: RegistryQuery { _, _ in
                throw RegistryQueryError("No registry read for an absent Git directory")
            }) { _ in Issue.record("A failed lookup cannot open a window") }
            Issue.record("Missing repository unexpectedly opened")
        } catch {}
        let report = router.inspect()
        #expect(report.windows.isEmpty)
        #expect(report.openings.count == 1)
        #expect(report.openings[0].status == .failed)
        #expect(report.openings[0].reason?.contains("not an available local Git repository") == true)
    }

    @Test func remoteTaskLinksReuseTheLocalRepositoryWindow() async throws {
        let router = WorkLinkRouter()
        let query = RegistryQuery { _, _ in #"{"id":"plan","locators":["plan"]}"# }
        let path = repositoryFixturePath()
        let workspace = try await router.openRepository(path: path, link: nil, query: query) { _ in }
        let window = UUID()
        var received: [LinkedWork] = []
        router.register(window, repository: workspace.id, focus: {}, inspect: windowInspection,
            perform: { DesktopActionOutcome(pane: $0.pane?.pane, tab: $0.pane?.tab) }, readText: { _ in throw RegistryQueryError("No terminal") }) { url in
            received.append(url)
        }
        for machine in ["local-owner", "peer-owner"] {
            let link = LinkedWork.task(TaskLink(issue: "task-\(machine)", repo: path, session: "session-\(machine)", diff: true,
                                machine: machine, repository: "plan"))
            _ = try await router.openRepository(path: path, link: link, query: query) { _ in
                Issue.record("Execution Machine must not create another repository window")
            }
        }
        while received.count < 2 { await Task.yield() }
        let tasks = received.compactMap { if case .task(let link) = $0 { link } else { nil } }
        #expect(tasks.map(\.machine) == ["local-owner", "peer-owner"])
        #expect(tasks.allSatisfy { $0.repository == "plan" && $0.diff })
        #expect(router.inspect().windows.map(\.window) == [window.uuidString])
    }

    @Test func batchedAddressesOpenInArrivalOrderAndRaiseOnlyTheLatest() async throws {
        let router = WorkLinkRouter(), barrier = LinkedDestinationBarrier()
        let path = repositoryFixturePath()
        let fast = RegistryQuery { _, _ in #"{"id":"plan","locators":["plan"]}"# }
        let workspace = try await router.openRepository(path: path, link: nil, query: fast) { _ in }
        var raised = 0
        var received: [String] = []
        router.register(UUID(), repository: workspace.id, focus: { raised += 1 }, inspect: windowInspection,
            perform: { DesktopActionOutcome(pane: $0.pane?.pane, tab: $0.pane?.tab) }, readText: { _ in throw RegistryQueryError("No terminal") }) { link in
            if case .task(let task) = link { received.append(task.issue) }
        }
        // The first read outlasts the second address's arrival.
        let slow = RegistryQuery { _, _ in
            await barrier.wait("first")
            return #"{"id":"plan","locators":["plan"]}"#
        }
        var finished = 0
        for (issue, query) in [("first", slow), ("second", fast)] {
            router.receiveAddress { isLatest in
                _ = try? await router.openRepository(path: path, link: .task(TaskLink(issue: issue, repo: path)),
                                                     query: query, raises: isLatest) { _ in
                    Issue.record("An open repository reuses its window")
                }
                finished += 1
            }
        }
        while !(await barrier.contains("first")) { await Task.yield() }
        await barrier.release("first")
        while finished < 2 || received.count < 2 { await Task.yield() }
        #expect(received == ["first", "second"], "both arrive, in arrival order")
        #expect(raised == 1, "only the final item raises its window")
    }

    @Test func plainRepositoryWaitsForRegistrationAndReusesTheWindow() async throws {
        let router = WorkLinkRouter()
        let query = RegistryQuery { _, _ in #"{"id":"plan","locators":["plan"]}"# }
        let workspace = try await router.openRepository(path: repositoryFixturePath(), link: nil, query: query) { _ in }
        #expect(router.inspect().windows.isEmpty)
        #expect(router.inspect().openings.first?.status == .opening)
        let id = UUID()
        router.register(id, repository: workspace.id, openingRequests: router.workspaceRequests(workspace.id), focus: {}, inspect: windowInspection,
                        perform: { DesktopActionOutcome(pane: $0.pane?.pane, tab: $0.pane?.tab) }, readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
        #expect(router.inspect().openings.first?.status == .usable)
        _ = try await router.openRepository(path: repositoryFixturePath(), link: nil, query: query) { _ in
            Issue.record("An open repository must reuse its window")
        }
        #expect(router.inspect().windows.map(\.window) == [id.uuidString])
        #expect(router.inspect().openings.first?.status == .usable)
    }

    @Test(arguments: ["success", "failure", "cancellation"])
    func obsoleteRepositoryLookupCannotOpenOrSettleANewerRequest(outcome: String) async throws {
        let router = WorkLinkRouter(), barrier = LinkedDestinationBarrier()
        let path = repositoryFixturePath()
        let old = Task {
            try await router.openRepository(path: path, link: nil, query: RegistryQuery { _, _ in
                await barrier.wait("lookup")
                if outcome == "failure" { throw RegistryQueryError("Old lookup failure") }
                return #"{"id":"old-plan","locators":["old-plan"]}"#
            }) { _ in Issue.record("An obsolete lookup must not open its window") }
        }
        while !(await barrier.contains("lookup")) { await Task.yield() }
        if outcome == "cancellation" { old.cancel() }
        let current = try await router.openRepository(path: path, link: nil, query: RegistryQuery { _, _ in #"{"id":"plan","locators":["plan"]}"# }) { _ in }
        await barrier.release("lookup")
        switch await old.result {
        case .success: Issue.record("Obsolete lookup unexpectedly succeeded")
        case .failure(let error): #expect(error is CancellationError)
        }
        #expect(router.inspect().openings.first?.status == .opening)
        router.register(UUID(), repository: "old-plan", focus: {}, inspect: windowInspection,
                        perform: { DesktopActionOutcome(pane: $0.pane?.pane, tab: $0.pane?.tab) }, readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
        #expect(router.inspect().openings.first?.status == .opening)
        router.register(UUID(), repository: current.id, openingRequests: router.workspaceRequests(current.id), focus: {}, inspect: windowInspection,
                        perform: { DesktopActionOutcome(pane: $0.pane?.pane, tab: $0.pane?.tab) }, readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
        #expect(router.inspect().openings.first?.status == .usable)
    }

    @Test(arguments: ["success", "failure", "cancellation"])
    func obsoleteSceneValidationCannotSettleNewRepositoryRequest(outcome: String) async throws {
        let router = WorkLinkRouter(), barrier = LinkedDestinationBarrier()
        let path = repositoryFixturePath()
        let query = RegistryQuery { _, _ in #"{"id":"plan","locators":["plan"]}"# }
        let workspace = try await router.openRepository(path: path, link: nil, query: query) { _ in }
        let oldRequests = router.workspaceRequests(workspace.id)
        let validation = Task {
            try await router.resolveWorkspace(workspace, query: RegistryQuery { _, _ in
                await barrier.wait("validate")
                if outcome == "failure" { throw RegistryQueryError("Old validation failure") }
                return #"{"id":"plan","locators":["plan"]}"#
            })
        }
        while !(await barrier.contains("validate")) { await Task.yield() }
        _ = try await router.openRepository(path: path, link: nil, query: query) { _ in }
        #expect(router.workspaceRequests(workspace.id) != oldRequests)
        if outcome == "cancellation" { validation.cancel() }
        await barrier.release("validate")
        switch await validation.result {
        case .success: Issue.record("Obsolete validation unexpectedly succeeded")
        case .failure(let error): #expect(error is CancellationError)
        }
        #expect(router.inspect().openings.first?.status == .opening)
        let stale = UUID()
        router.register(stale, repository: workspace.id, openingRequests: oldRequests, focus: {}, inspect: windowInspection,
                        perform: { DesktopActionOutcome(pane: $0.pane?.pane, tab: $0.pane?.tab) }, readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
        #expect(router.inspect().openings.first?.status == .opening)
        router.remove(stale, repository: workspace.id)
        _ = try await router.resolveWorkspace(workspace, query: query)
        router.register(UUID(), repository: workspace.id, openingRequests: router.workspaceRequests(workspace.id), focus: {}, inspect: windowInspection,
                        perform: { DesktopActionOutcome(pane: $0.pane?.pane, tab: $0.pane?.tab) }, readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
        #expect(router.inspect().openings.first?.status == .usable)
    }

    @Test func requestArrivingBetweenValidationAndRegistrationUsesTheRetainedShell() async throws {
        let router = WorkLinkRouter()
        let path = repositoryFixturePath()
        let query = RegistryQuery { _, _ in #"{"id":"plan","locators":["plan"]}"# }
        let workspace = try await router.openRepository(path: path, link: nil, query: query) { _ in }
        let validated = router.workspaceRequests(workspace.id)
        _ = try await router.resolveWorkspace(workspace, query: query)
        _ = try await router.openRepository(path: path, link: nil, query: query) { _ in }
        let current = router.workspaceRequests(workspace.id)
        let id = UUID()
        router.register(id, repository: workspace.id, openingRequests: validated, focus: {}, inspect: windowInspection,
                        perform: { DesktopActionOutcome(pane: $0.pane?.pane, tab: $0.pane?.tab) }, readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
        #expect(router.inspect().openings.first?.status == .opening)
        router.confirmRegisteredWorkspace(workspace.id, requests: validated)
        #expect(router.inspect().openings.first?.status == .opening)
        router.confirmRegisteredWorkspace(workspace.id, requests: current)
        #expect(router.inspect().openings.first?.status == .usable)
        #expect(router.inspect().windows.map(\.window) == [id.uuidString])
    }

    @Test func sceneValidationFailureRemainsFailedAfterDelayedRegistration() async throws {
        let router = WorkLinkRouter()
        let workspace = try await router.openRepository(path: repositoryFixturePath(), link: nil,
            query: RegistryQuery { _, _ in #"{"id":"plan","locators":["plan"]}"# }) { _ in }
        do {
            _ = try await router.resolveWorkspace(workspace, query: RegistryQuery { _, _ in #"{"id":"different-plan","locators":["different-plan"]}"# })
            Issue.record("A changed plan must not restore this workspace")
        } catch {}
        #expect(router.inspect().windows.isEmpty)
        #expect(router.inspect().openings.first?.status == .failed)
        let receipt = router.inspect().openings
        router.register(UUID(), repository: workspace.id, openingRequests: router.workspaceRequests(workspace.id), focus: {}, inspect: windowInspection,
                        perform: { DesktopActionOutcome(pane: $0.pane?.pane, tab: $0.pane?.tab) }, readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
        #expect(router.inspect().openings == receipt)
    }

    @Test func repositoryAssociationRetainsDraftsPanesAndInFlightDelivery() async throws {
        let router = WorkLinkRouter(), barrier = LinkedDestinationBarrier()
        let path = repositoryFixturePath()
        let before = RegistryQuery { _, _ in #"{"id":"old-plan","locators":["old-plan"]}"# }
        let after = RegistryQuery { _, _ in #"{"id":"plan","locators":["old-plan","plan"]}"# }
        let first = LinkedWork.task(TaskLink(issue: "FIRST", repo: path))
        let second = LinkedWork.task(TaskLink(issue: "SECOND", repo: path))
        let scene = try await router.openRepository(path: path, link: first, query: before) { _ in }
        let deliveredFirst = LinkedWork.task(TaskLink(issue: "FIRST", repo: scene.path))
        let deliveredSecond = LinkedWork.task(TaskLink(issue: "SECOND", repo: scene.path))
        let registry = SessionsWorkspaceRegistry(localMachineId: "fixture-machine")
        let identity = WorkspaceIdentity(machineId: "fixture-machine", worktree: path)
        let oldKey = WorkWorkspaceKey(repository: "old-plan", kind: .repository, id: "old-plan")
        let retained = registry.open(oldKey, title: "Repository", location: identity), store = retained.multiplexer
        store.load(sessionId: "retained", location: identity)
        let pane = store.focusedPane
        let document = retained.files(issue: "TASK", cwd: path).document("note.txt")
        document.editor.string = "unfinished draft"
        document.editor.setSelectedRange(NSRange(location: 2, length: 4))
        let layout = store.layout, focus = store.focusedPaneId
        #if canImport(GhosttyKit)
        let terminal = try #require(pane.terminal)
        let view = registry.surfaces.view(for: terminal)
        defer { registry.surfaces.release(terminal) }
        #endif
        let window = UUID()
        let (model, _) = destinationModel(query: RegistryQuery { _, _ in throw RegistryQueryError("No Work reads") })
        model.bindWorkspaces(registry, repository: "old-plan")
        var received: [LinkedWork] = []
        router.register(window, repository: scene.id, openingRequests: router.workspaceRequests(scene.id), focus: {},
            inspect: { repository, incarnation in
                model.inspectDesktop(repository: repository, window: incarnation, workspaces: registry)
            }, perform: { try DesktopAction.perform($0, model: model, workspaces: registry) },
            readText: { try registry.readText($0) }) { url in
                received.append(url)
                if url == deliveredFirst { await barrier.wait("delivery") }
            }
        while !(await barrier.contains("delivery")) { await Task.yield() }
        let rebound = try await router.openRepository(path: path, link: second, query: after) { _ in
            Issue.record("Association must reuse the retained window")
        }
        #expect(rebound == scene)
        for _ in 0..<2 {
            _ = try await router.openRepository(path: path, link: nil, query: after) { _ in
                Issue.record("Repeated opens must reuse the retained window")
            }
        }
        let report = router.inspect()
        #expect(report.windows.count == 1)
        #expect(report.windows[0].repository == "plan")
        #expect(report.windows[0].window != window.uuidString)
        #expect(received == [deliveredFirst])
        #expect(registry.retained(WorkWorkspaceKey(repository: "plan", kind: .repository, id: "plan")) === retained)
        #if canImport(GhosttyKit)
        #expect(registry.surfaces.view(for: terminal) === view)
        #expect(registry.surfaces.programStatus(for: terminal) === view.programStatus)
        #endif
        #expect(store.layout == layout)
        #expect(store.focusedPaneId == focus)
        #expect(document.editor.string == "unfinished draft")
        #expect(document.editor.selectedRange() == NSRange(location: 2, length: 4))
        let stale = DesktopPaneTarget(repository: "old-plan", window: window.uuidString,
            work: oldKey, pane: pane.id, incarnation: pane.incarnation, tab: nil)
        #expect(throws: DesktopTargetError.self) {
            try router.perform(DesktopActionRequest("hide", at: stale))
        }
        let current = DesktopPaneTarget(repository: "plan", window: report.windows[0].window,
            work: retained.key, pane: pane.id, incarnation: pane.incarnation, tab: nil)
        _ = try router.perform(DesktopActionRequest("hide", at: current))
        _ = try router.perform(DesktopActionRequest("show", at: current))
        #expect(store.layout == layout)
        await barrier.release("delivery")
        while received.count < 2 { await Task.yield() }
        #expect(received == [deliveredFirst, deliveredSecond])
        #expect(document.editor.string == "unfinished draft")
        _ = try await router.openRepository(path: path, link: nil, query: RegistryQuery { _, _ in
            #"{"id":"old-plan","locators":["old-plan","plan"]}"#
        }) { _ in Issue.record("Binding back must also reuse the window") }
        #expect(router.inspect().windows.first?.repository == "old-plan")
        #expect(throws: DesktopTargetError.self) {
            try router.perform(DesktopActionRequest("hide", at: stale))
        }
        #expect(throws: DesktopTargetError.self) {
            try router.perform(DesktopActionRequest("hide", at: current))
        }
        router.remove(window, repository: scene.id)
        #expect(router.inspect().windows.isEmpty)
    }

    @Test func associationBeforeRegistrationKeepsOneSceneAndRejectsLateLookup() async throws {
        let router = WorkLinkRouter(), barrier = LinkedDestinationBarrier()
        let path = repositoryFixturePath()
        var opened: [RepositoryWorkspace] = []
        let scene = try await router.openRepository(path: path, link: nil,
            query: RegistryQuery { _, _ in #"{"id":"old-plan","locators":["old-plan"]}"# }) { opened.append($0) }
        let stale = Task {
            try await router.openRepository(path: path, link: nil, query: RegistryQuery { _, _ in
                await barrier.wait("lookup")
                return #"{"id":"old-plan","locators":["old-plan"]}"#
            }) { opened.append($0) }
        }
        while !(await barrier.contains("lookup")) { await Task.yield() }
        let after = RegistryQuery { _, _ in #"{"id":"plan","locators":["old-plan","plan"]}"# }
        _ = try await router.openRepository(path: path, link: nil, query: after) { opened.append($0) }
        await barrier.release("lookup")
        switch await stale.result {
        case .success: Issue.record("A stale lookup cannot change repository association")
        case .failure(let error): #expect(error is CancellationError)
        }
        #expect(Set(opened).count == 1)
        #expect(opened.first == scene)
        // Scene restoration accepts retained identity without unmounting/rekeying it.
        #expect(try await router.resolveWorkspace(scene, query: after) == scene)
        let restoredAlias = RepositoryWorkspace(id: "plan", path: path)
        #expect(try await router.resolveWorkspace(restoredAlias, query: after) == scene)
        let window = UUID()
        router.register(window, repository: scene.id, openingRequests: router.workspaceRequests(scene.id), focus: {},
            inspect: windowInspection, perform: { DesktopActionOutcome(pane: $0.pane?.pane, tab: $0.pane?.tab) },
            readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
        #expect(router.inspect().windows.map(\.repository) == ["plan"])
        #expect(router.inspect().openings.first?.status == .usable)
        _ = try await router.openRepository(path: path, link: nil, query: after) { _ in
            Issue.record("Concurrent opens must converge before and after registration")
        }
        #expect(router.inspect().windows.map(\.window) == [window.uuidString])
    }

    @Test(arguments: [false, true])
    func restoredAliasCannotUndoBindingBackToTheSamePlan(registered: Bool) async throws {
        let router = WorkLinkRouter(), barrier = LinkedDestinationBarrier()
        let path = repositoryFixturePath()
        let before = RegistryQuery { _, _ in #"{"id":"old-plan","locators":["old-plan","plan"]}"# }
        let after = RegistryQuery { _, _ in #"{"id":"plan","locators":["old-plan","plan"]}"# }
        let scene = try await router.openRepository(path: path, link: nil, query: before) { _ in }
        let window = UUID()
        if registered {
            router.register(window, repository: scene.id, focus: {}, inspect: windowInspection,
                perform: { DesktopActionOutcome(pane: $0.pane?.pane, tab: $0.pane?.tab) }, readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
        }
        let validation = Task {
            try await router.resolveWorkspace(RepositoryWorkspace(id: "plan", path: path), query: RegistryQuery { _, _ in
                await barrier.wait("restore")
                return #"{"id":"plan","locators":["old-plan","plan"]}"#
            })
        }
        while !(await barrier.contains("restore")) { await Task.yield() }
        _ = try await router.openRepository(path: path, link: nil, query: after) { _ in }
        _ = try await router.openRepository(path: path, link: nil, query: before) { _ in }
        let reboundWindow = router.inspect().windows.first?.window
        await barrier.release("restore")
        #expect(try await validation.value == scene)
        if !registered {
            router.register(window, repository: scene.id, focus: {}, inspect: windowInspection,
                perform: { DesktopActionOutcome(pane: $0.pane?.pane, tab: $0.pane?.tab) }, readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
        }
        #expect(router.inspect().windows.map(\.repository) == ["old-plan"])
        #expect(router.inspect().windows.first?.window == (reboundWindow ?? window.uuidString))
    }

    @Test(arguments: [false, true])
    func delayedLocatorOpenCannotUndoRepositoryAssociation(alreadyOpen: Bool) async throws {
        let router = WorkLinkRouter(), barrier = LinkedDestinationBarrier()
        let path = repositoryFixturePath()
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let parent = directory.appendingPathComponent("parent")
        let repository = URL(fileURLWithPath: path)
        try FileManager.default.createSymbolicLink(at: parent, withDestinationURL: repository.deletingLastPathComponent())
        let alias = parent.appendingPathComponent(repository.lastPathComponent)
        try #require(RepositoryPath().mainRepository(alias) != nil)
        let window = UUID()
        var received: [LinkedWork] = []
        let register: (RepositoryWorkspace) -> Void = { scene in
            router.register(window, repository: scene.id, openingRequests: router.workspaceRequests(scene.id),
                focus: {}, inspect: windowInspection,
                perform: { DesktopActionOutcome(pane: $0.pane?.pane, tab: $0.pane?.tab) }, readText: { _ in throw RegistryQueryError("No terminal") }) { received.append($0) }
        }
        if alreadyOpen {
            _ = try await router.openRepository(path: path, link: nil,
                query: RegistryQuery { _, _ in #"{"id":"old-plan","locators":["old-plan"]}"# }, openWindow: register)
        }
        let link = LinkedWork.task(TaskLink(issue: "PENDING", repo: alias.path))
        let delayed = Task {
            try await router.openRepository(path: alias.path, link: link, query: RegistryQuery { _, _ in
                await barrier.wait("alias")
                return #"{"id":"old-plan","locators":["old-plan"]}"#
            }) { _ in Issue.record("A locator must reuse its repository window") }
        }
        while !(await barrier.contains("alias")) { await Task.yield() }
        let scene = try await router.openRepository(path: path, link: nil,
            query: RegistryQuery { _, _ in #"{"id":"plan","locators":["old-plan","plan"]}"# }) { workspace in
                if alreadyOpen { Issue.record("Association must reuse the existing window") }
                register(workspace)
            }
        let rebound = router.inspect().windows
        await barrier.release("alias")
        try #require(try await delayed.value == scene)
        #expect(router.inspect().windows == rebound)
        try #require(router.inspect().windows.count == 1)
        while received.isEmpty { await Task.yield() }
        #expect(received == [.task(TaskLink(issue: "PENDING", repo: scene.path))])
        #expect(router.inspect().openings.allSatisfy { $0.status == .usable })
    }

    @Test func repositoryMoveRetainsWindowAndSessionStore() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let before = directory.appendingPathComponent("before")
        let after = directory.appendingPathComponent("after")
        try FileManager.default.createDirectory(at: before, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let git = Foundation.Process()
        git.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        git.arguments = ["init", "-q", before.path]
        try git.run()
        git.waitUntilExit()
        #expect(git.terminationStatus == 0)
        let query = RegistryQuery { args, cwd in
            if args.contains("--path") { return after.path }
            if cwd == before.path.normalizedFilePath && FileManager.default.fileExists(atPath: after.path) {
                return #"{"id":"unrelated","locators":["unrelated"]}"#
            }
            return #"{"id":"retained-plan","locators":["retained-plan"]}"#
        }
        let router = WorkLinkRouter()
        let workspace = try await router.openRepository(path: before.path, link: nil, query: query) { _ in }
        let window = UUID()
        router.register(window, repository: workspace.id, focus: {}, inspect: windowInspection,
            perform: { DesktopActionOutcome(pane: $0.pane?.pane, tab: $0.pane?.tab) }, readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
        let registry = SessionsWorkspaceRegistry(localMachineId: "local")
        let store = registry.sessionStore(repository: workspace.id, repoPath: before.path, query: query)
        try FileManager.default.moveItem(at: before, to: after)
        let replacement = Foundation.Process()
        replacement.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        replacement.arguments = ["init", "-q", before.path]
        try replacement.run()
        replacement.waitUntilExit()
        #expect(replacement.terminationStatus == 0)
        #expect(try await router.resolveWorkspace(workspace, query: query).path == after.path.normalizedFilePath)
        let moved = try await router.openRepository(path: after.path, link: nil, query: query) { _ in
            Issue.record("A move must retain the existing window")
        }
        #expect(moved.id == workspace.id)
        #expect(router.repositoryPath(workspace.id) == moved.path)
        #expect(router.inspect().windows.map(\.window) == [window.uuidString])
        #expect(try await router.resolveWorkspace(workspace, query: query).path == moved.path)
        #expect(registry.sessionStore(repository: workspace.id, repoPath: moved.path, query: query) === store)
        #expect(store.repoPath == WaveOrigin.resolve(moved.path))
    }

    private func repositoryFixturePath() -> String {
        URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().path
    }

    private func windowInspection(_ repository: String, _ id: UUID) -> DesktopWindowInspection {
        DesktopWindowInspection(repository: repository, window: id.uuidString, path: nil,
            destination: .work(WorkWorkspaceKey(repository: "repo", kind: .repository, id: "repo")), history: DesktopHistory(back: 0, forward: 0), reading: "loading", reason: nil,
            task: nil, session: nil, list: [], actions: [], workspaces: [], openWork: [], view: nil, viewParts: [], opening: nil)
    }

    @Test func inspectionIncludesOnlyRegisteredWindowIncarnationsWithoutFocusing() {
        let router = WorkLinkRouter()
        let first = UUID(), replacement = UUID()
        var focused = false
        router.register(first, repository: "plan", focus: { focused = true }, inspect: windowInspection, perform: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { _ in }
        #expect(router.inspect().windows.map(\.window) == [first.uuidString])
        router.register(replacement, repository: "plan", focus: { focused = true }, inspect: windowInspection, perform: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { _ in }
        router.remove(first, repository: "plan")
        #expect(router.inspect().windows.map(\.window) == [replacement.uuidString])
        router.remove(replacement, repository: "plan")
        #expect(router.inspect().windows.isEmpty)
        #expect(!focused)
    }

    @Test func coldRepositoriesRegisterInReverseOrderWithoutLosingDestinations() async throws {
        let router = WorkLinkRouter()
        let a = LinkedWork.task(TaskLink(issue: "A", repo: nil))
        let a2 = LinkedWork.task(TaskLink(issue: "A2", repo: nil))
        let b = LinkedWork.task(TaskLink(issue: "B", repo: nil))
        #expect(!router.deliver(a, repository: "plan-a"))
        #expect(!router.deliver(b, repository: "plan-b"))
        #expect(!router.deliver(a2, repository: "plan-a"))
        var first: [LinkedWork] = []
        var second: [LinkedWork] = []
        router.register(UUID(), repository: "plan-b", focus: {}, inspect: windowInspection, perform: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { second.append($0) }
        while second.isEmpty { await Task.yield() }
        #expect(second == [b])
        #expect(first.isEmpty)
        router.register(UUID(), repository: "plan-a", focus: {}, inspect: windowInspection, perform: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { first.append($0) }
        while first.count < 2 { await Task.yield() }
        #expect(first == [a, a2])
        #expect(second == [b])
    }

    @Test func samePlanReusesItsRetainedWorkspaceRegardlessOfFocusOrLocator() async throws {
        let a = RepositoryWorkspace(id: "plan", path: "/machine-a/repo")
        let b = RepositoryWorkspace(id: "plan", path: "/machine-b/repo")
        #expect(Set([a, b]).count == 1)
        #expect(a != RepositoryWorkspace(id: "another-plan", path: a.path))
        let router = WorkLinkRouter()
        var focused: [String] = []
        var received: [LinkedWork] = []
        let url = LinkedWork.task(TaskLink(issue: "A", repo: nil))
        router.register(UUID(), repository: a.id, focus: { focused.append("retained") }, inspect: windowInspection, perform: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { received.append($0) }
        router.register(UUID(), repository: "other", focus: { focused.append("other") }, inspect: windowInspection, perform: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { _ in
            Issue.record("A repository must never receive another repository's link")
        }
        #expect(router.deliver(url, repository: b.id))
        #expect(router.deliver(nil, repository: a.id))
        #expect(focused == ["retained", "retained"])
        while received.isEmpty { await Task.yield() }
        #expect(received == [url])
    }

    @Test func lateWindowRemovalDoesNotRemoveItsReplacement() async throws {
        let router = WorkLinkRouter()
        let old = UUID(), replacement = UUID()
        router.register(old, repository: "plan", focus: {}, inspect: windowInspection, perform: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { _ in }
        var received: [LinkedWork] = []
        router.register(replacement, repository: "plan", focus: {}, inspect: windowInspection, perform: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { received.append($0) }
        router.remove(old, repository: "plan")
        let url = LinkedWork.task(TaskLink(issue: "A", repo: nil))
        #expect(router.deliver(url, repository: "plan"))
        while received.isEmpty { await Task.yield() }
        #expect(received == [url])
        router.remove(replacement, repository: "plan")
        #expect(!router.deliver(url, repository: "plan"))
    }

    @Test func delayedDeliveryKeepsOneQueueAcrossReceiverReplacement() async throws {
        let router = WorkLinkRouter()
        let first = LinkedWork.task(TaskLink(issue: "first", repo: nil))
        let second = LinkedWork.task(TaskLink(issue: "second", repo: nil))
        let third = LinkedWork.task(TaskLink(issue: "third", repo: nil))
        let other = LinkedWork.task(TaskLink(issue: "other", repo: nil))
        let old = UUID(), replacement = UUID()
        let barrier = LinkedDestinationBarrier()
        var delivered: [LinkedWork] = []
        router.register(old, repository: "plan", focus: {}, inspect: windowInspection, perform: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { link in
            await barrier.wait("first")
            guard !Task.isCancelled else { return }
            delivered.append(link)
        }
        router.deliver(first, repository: "plan")
        router.deliver(second, repository: "plan")
        while !(await barrier.contains("first")) { await Task.yield() }
        #expect(delivered.isEmpty)
        router.remove(old, repository: "plan")
        #expect(!router.deliver(third, repository: "plan"))
        router.register(replacement, repository: "plan", focus: {}, inspect: windowInspection, perform: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { delivered.append($0) }
        router.remove(old, repository: "plan")

        // A slow repository never blocks another repository's opening.
        router.register(UUID(), repository: "other", focus: {}, inspect: windowInspection, perform: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { delivered.append($0) }
        router.deliver(other, repository: "other")
        while delivered.count < 4 { await Task.yield() }
        #expect(delivered.filter { $0 != other } == [first, second, third])
        #expect(delivered.contains(other))
        // The replacement finishes even while the obsolete read is suspended.
        await barrier.release("first")
        for _ in 0..<10 { await Task.yield() }
        #expect(delivered.count == 4)
    }

    @Test func reattachingTheSameReceiverDoesNotLetItsCanceledReadClearTheNewQueue() async throws {
        let router = WorkLinkRouter(), id = UUID()
        let first = LinkedWork.task(TaskLink(issue: "first", repo: nil))
        let second = LinkedWork.task(TaskLink(issue: "second", repo: nil))
        let barrier = LinkedDestinationBarrier()
        var delivered: [LinkedWork] = []
        router.register(id, repository: "plan", focus: {}, inspect: windowInspection, perform: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { _ in await barrier.wait("old") }
        router.deliver(first, repository: "plan")
        while !(await barrier.contains("old")) { await Task.yield() }
        router.remove(id, repository: "plan")
        router.register(id, repository: "plan", focus: {}, inspect: windowInspection, perform: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { link in
            if link == first { await barrier.wait("new") }
            delivered.append(link)
        }
        while !(await barrier.contains("new")) { await Task.yield() }
        await barrier.release("old")
        for _ in 0..<10 { await Task.yield() }
        router.deliver(second, repository: "plan")
        for _ in 0..<10 { await Task.yield() }
        #expect(delivered.isEmpty)
        await barrier.release("new")
        while delivered.count < 2 { await Task.yield() }
        #expect(delivered == [first, second])
    }

    @Test(arguments: [false, true])
    func closedWindowDoesNotPublishDelayedOpening(fails: Bool) async throws {
        let data = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let task = try #require(snapshot.waves.first?.tasks.items.first)
        let exact = try oneTask(data, taskId: task.id)
        let barrier = LinkedDestinationBarrier()
        // The bound workspace restores its pane layout through this query too;
        // only the Task read may hold the barrier.
        let (model, _) = destinationModel(query: RegistryQuery { args, _ in
            guard args.contains("--task") else { throw RegistryQueryError("Only the Task read is delayed") }
            await barrier.wait("read")
            if fails { throw RegistryQueryError("Delayed failure") }
            return exact
        }, repoPath: "/origin")
        model.navigation.selectedSessionId = "retained-session"
        let link = TaskLink(issue: task.task.identifier, repo: nil)
        let opening = Task { await model.openTaskLink(link) }
        while !(await barrier.contains("read")) { await Task.yield() }
        opening.cancel()
        await barrier.release("read")
        await opening.value
        #expect(model.repoPath == "/origin")
        #expect(model.selection == nil)
        #expect(model.navigation.selectedSessionId == "retained-session")
        #expect(!model.showsTaskLink)
    }

    @Test func repositoryNavigationLeavesTheOriginSelectionAndDraftWorkspaceAlone() async throws {
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(try fixture().utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let (model, _) = destinationModel(query: RegistryQuery { _, _ in "" }, repoPath: "/origin")
        model.navigation.selectedSessionId = "retained-session"
        var requests: [(String, LinkedWork?)] = []
        model.openRepository = { requests.append(($0, $1)) }
        model.setRepoPath("/other")
        model.openTaskDestination(wave: wave, task: task)
        #expect(model.repoPath == "/origin")
        #expect(model.navigation.selectedSessionId == "retained-session")
        #expect(requests.map { $0.0 } == ["/other", wave.wave.repo])
        #expect(requests.last?.1 == .task(TaskLink(issue: task.task.identifier, repo: wave.wave.repo)))
    }

    @Test(arguments: [false, true])
    func overlappingLinksRespectLaterNavigation(navigateAway: Bool) async throws {
        let data = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let wave = try #require(snapshot.waves.first)
        let tasks = snapshot.waves.flatMap { $0.tasks.items }
        let first = try #require(tasks.first)
        let second = try #require(tasks.last)
        let responses = [first.task.identifier: try oneTask(data, taskId: first.id),
                         second.task.identifier: try oneTask(data, taskId: second.id)]
        let barrier = LinkedDestinationBarrier()
        let query = RegistryQuery { args, _ in
            if let index = args.firstIndex(of: "--task") {
                let issue = args[index + 1]
                await barrier.wait(issue)
                return responses[issue]!
            }
            if args.prefix(2) == ["desktop", "plan"] { return data }
            if args.prefix(2) == ["desktop", "sessions"] { return #"{"entries":[],"next":null}"# }
            if args.first == "wave" { return "[]" }
            throw RegistryQueryError("No mutation permitted")
        }
        let (model, _) = destinationModel(query: query, repoPath: wave.wave.repo)
        await model.refresh()
        model.select(.wave(id: wave.wave.id))
        let firstURL = TaskLink(issue: first.task.identifier, repo: nil)
        let secondURL = TaskLink(issue: second.task.identifier, repo: nil)
        let firstRead = Task { await model.openTaskLink(firstURL) }
        while !(await barrier.contains(first.task.identifier)) { await Task.yield() }
        let secondRead = Task { await model.openTaskLink(secondURL) }
        while !(await barrier.contains(second.task.identifier)) { await Task.yield() }
        if navigateAway { model.select(.wave(id: wave.wave.id)) }
        await barrier.release(second.task.identifier)
        await secondRead.value
        await barrier.release(first.task.identifier)
        await firstRead.value
        #expect(model.selection == (navigateAway ? .wave(id: wave.wave.id) : .task(id: second.id)))
        #expect(!model.showsTaskLink)
        #expect(model.navigation.recentDestinations.map(\.id) == (navigateAway ? [] : [.task(second.id)]))
    }

    private func fixture() throws -> String {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        return try String(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json"), encoding: .utf8)
    }

    private func oneTask(_ data: String, taskId: String) throws -> String {
        var snapshot = try #require(JSONSerialization.jsonObject(with: Data(data.utf8)) as? [String: Any])
        var inventory = try #require(snapshot["tasks"] as? [String: Any])
        let tasks = try #require(inventory["items"] as? [[String: Any]])
        inventory["items"] = tasks.filter { ($0["task"] as? [String: Any])?["id"] as? String == taskId }
        snapshot["tasks"] = inventory
        return String(decoding: try JSONSerialization.data(withJSONObject: snapshot), as: UTF8.self)
    }
}

private actor LinkedDestinationBarrier {
    private var pending: [String: CheckedContinuation<Void, Never>] = [:]
    func contains(_ key: String) -> Bool { pending[key] != nil }
    func wait(_ key: String) async {
        await withCheckedContinuation { continuation in
            // Replacing a waiter would strand it forever; fail the test instead.
            if let earlier = pending.updateValue(continuation, forKey: key) {
                Issue.record("Two reads are waiting at barrier \"\(key)\"")
                earlier.resume()
            }
        }
    }
    func release(_ key: String) { pending.removeValue(forKey: key)?.resume() }
}

private actor DestinationReadCounter {
    private var count = 0
    func next() -> Int { count += 1; return count }
}
#endif
