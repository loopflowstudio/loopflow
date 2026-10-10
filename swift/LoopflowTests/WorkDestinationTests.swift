#if os(macOS)
import AppKit
import Foundation
import Observation
import Testing
import ViewInspector
import os
@testable import Loopflow
@testable import LoopflowMac

@Suite("Work destinations", .serialized)
@MainActor
struct WorkDestinationTests {
    @Test func taskLinksRetainLiteralTargetsWhenChangingRepositoryLocator() throws {
        let original = TaskLink(issue: "LOO-427", repo: "/src/a #?&% repo", session: "session+&%", diff: true)
        let parsed = try TaskLink(url: #require(original.url))
        #expect(parsed == original)
        let relocated = TaskLink(issue: parsed.issue, repo: "/peer/other #repo", session: parsed.session, diff: parsed.diff)
        #expect(try TaskLink(url: #require(relocated.url)) == relocated)
        #expect(try TaskLink(url: #require(TaskLink(issue: "LOO-427", repo: nil).url))
            == TaskLink(issue: "LOO-427", repo: nil))
    }

    @Test(arguments: ["recorded", "stale", "unavailable", "unrecorded", "wrong-checkout"])
    func remoteOpeningKeepsLocalPlanningAndRetainedDrafts(outcome: String) async throws {
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
        surface["actions"] = sessionActionFixture(state: "closed")
        surface["open_argv"] = ["ssh", "-tt", "peer", "owned-fixture"]
        record = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: surface))
        let encoded = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let query = RegistryQuery { args, cwd in
            if args.starts(with: ["repo", "identity"]) { return #"{"id":"selected-plan","locators":["selected-plan","repository"]}"# }
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
            if command.starts(with: ["session", "list"]) { return #"{"entries":[\#(encoded)],"next":null}"# }
            if command.starts(with: ["session", "connect"]) { return encoded }
            throw RegistryQueryError("No checkout, provider launch or transfer permitted")
        }
        let model = WorkModel(query: query, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        let registry = SessionsWorkspaceRegistry(localMachineId: localIdentity.machineId)
        let local = registry.workspace(for: localIdentity)
        local.multiplexer.newShell(command: ["retained-local-shell"])
        let layout = local.multiplexer.layout
        let files = local.files(taskId: task.id, issue: taskID, cwd: localIdentity.worktree, query: query)
        let document = files.document("draft.txt")
        document.editor.string = "unfinished local draft"
        let link = TaskLink(issue: taskID, repo: wave.wave.repo, session: record.id, diff: true,
                            machine: "peer", repository: "repository")
        #expect(try TaskLink(url: #require(link.url)) == link)
        await model.openTaskLink(try #require(link.url))
        #expect(model.repoPath == wave.wave.repo)
        #expect(model.roadmap.value == snapshot)
        #expect(local.multiplexer.layout == layout)
        #expect(document.editor.string == "unfinished local draft")
        if outcome == "recorded" {
            let request = try #require(model.linkedSession)
            #expect(request.record.id == record.id)
            #expect(request.location?.machineID == "peer")
            #expect(model.selection == .task(id: task.id))
            #expect(model.navigation.preparedTaskWorktrees[task.id] == remoteIdentity)
            #expect(model.taskOpening?.status == .opening)
            let remote = registry.workspace(for: remoteIdentity)
            remote.multiplexer.load(sessionId: record.id)
            let companion = registry.showChanges(task: task, in: remoteIdentity,
                query: query.onMachine("peer", repository: "repository"), executionTask: remoteID)
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
            #expect(remote.multiplexer.layout.allPanes.count == 2)
            #expect(local.multiplexer.layout == layout)
        } else {
            #expect(model.linkedSession == nil)
            #expect(model.taskOpening?.status == .failed)
            #expect(model.navigation.preparedTaskWorktrees[task.id] == nil)
            #expect(registry.paths == [localIdentity])
        }
    }

    @Test func resolvingTaskKeepsTheWorkspaceVisible() async throws {
        let data = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let task = try #require(snapshot.waves.first?.tasks.items.first)
        let exact = try oneTask(data, taskId: task.id)
        let barrier = LinkedDestinationBarrier()
        let model = WorkModel(query: RegistryQuery { _, _ in
            await barrier.wait("read")
            return exact
        })
        let url = try #require(URL(string: "loopflow://task/\(task.task.identifier)"))
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
        let model = WorkModel(query: RegistryQuery { _, _ in
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
        var url = try #require(URLComponents(string: "loopflow://task/\(task.task.identifier)"))
        url.queryItems = [URLQueryItem(name: "repo", value: wave.wave.repo)]
        await model.openTaskLink(try #require(url.url))
        #expect(model.selection == .task(id: selectedID))
        #expect(model.navigation.selectedSessionId == "retained-conversation")
        #expect(model.navigation.content == .terminals)
        #expect(model.taskLinkReading.errorMessage == nil)
        #expect(!model.showsTaskLink)

        // Reopening what is already open changes nothing, so nothing redraws.
        let invalidations = OSAllocatedUnfairLock(initialState: 0)
        withObservationTracking {
            _ = (model.repoPath, model.taskLinkURL, model.showsTaskLink, model.linkedSession,
                 model.navigation.selectedTaskEvidence?.task, model.navigation.recentDestinations)
        } onChange: {
            invalidations.withLock { $0 += 1 }
        }
        await model.openTaskLink(try #require(url.url))
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
        let model = WorkModel(query: RegistryQuery { _, _ in
            await reads.next() == 1 ? exact : empty
        }, repoPath: wave.wave.repo)
        var url = try #require(URLComponents(string: "loopflow://task/\(task.task.identifier)"))
        url.queryItems = [URLQueryItem(name: "repo", value: wave.wave.repo)]
        await model.openTaskLink(try #require(url.url))
        model.applyFixture(roadmap: .available(removed), waves: .available([]),
                           workActivity: .loading, repos: [])
        await model.openTaskLink(try #require(url.url))
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
        let model = WorkModel(query: RegistryQuery { args, _ in
            guard args.first == "session" else {
                throw RegistryQueryError("Planning is already available")
            }
            if args.contains("--after") { return #"{"entries":[\#(otherEncoded)],"next":null}"# }
            return #"{"entries":[\#(encoded)],"next":"remaining-history"}"#
        }, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]),
                           workActivity: .loading, repos: [])
        var url = try #require(URLComponents(string: "loopflow://task/\(task.task.identifier)"))
        url.queryItems = [URLQueryItem(name: "repo", value: wave.wave.repo), URLQueryItem(name: "session", value: record.id)]
        await model.openTaskLink(try #require(url.url))
        #expect(model.linkedSession?.record.id == record.id)
        #expect(model.navigation.selectedSessionId == record.id)
        #expect(model.taskLinkReading.errorMessage == nil)
        // Reopening from retained evidence must preserve the conversation too.
        await model.openTaskLink(try #require(url.url))
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
            guard args.starts(with: ["session", "list"]) else { throw RegistryQueryError("No launch or planning mutation permitted") }
            return #"{"entries":[\#(encoded)],"next":null}"#
        }
        let model = WorkModel(query: query, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        let sessionOnly = TaskLink(issue: task.task.identifier, repo: wave.wave.repo, session: record.id)
        await model.openTaskLink(try #require(sessionOnly.url))
        #expect(model.linkedSession?.record.id == record.id)
        #expect(model.linkedSession?.changesTask == nil)
        // The same Session with a companion is a different pending destination,
        // even if SwiftUI has not consumed the first request yet.
        let link = TaskLink(issue: try #require(task.runtime?.workId), repo: wave.wave.repo, session: record.id, diff: true)
        await model.openTaskLink(try #require(link.url))
        #expect(model.linkedSession?.record.id == record.id)
        #expect(model.linkedSession?.changesTask?.id == task.id)
        #expect(model.taskOpening?.status == .opening) // selection is not native readiness
        let registry = SessionsWorkspaceRegistry()
        let workspace = registry.workspace(for: identity)
        workspace.multiplexer.load(sessionId: record.id)
        let terminal = try #require(workspace.multiplexer.layout.allPanes.first)
        registry.showChanges(task: task, in: identity, query: query)
        let files = workspace.files(taskId: try #require(task.runtime?.workId), issue: task.task.identifier, cwd: identity.worktree, query: query)
        files.selection = "retained-draft.rs"
        registry.showChanges(task: task, in: identity, query: query)
        #expect(workspace.multiplexer.layout.allPanes.count == 2)
        #expect(workspace.multiplexer.layout.pane(for: terminal.id) == terminal)
        #expect(workspace.multiplexer.focusedPaneId == terminal.id)
        #expect(files.selection == "retained-draft.rs")
        #expect(files.showsChanges)
        let failure = TaskLink(issue: task.task.identifier, repo: wave.wave.repo, session: "missing", diff: true)
        await model.openTaskLink(try #require(failure.url))
        #expect(model.taskOpening?.status == .failed)
        #expect(model.taskOpening?.url == failure.url?.absoluteString)
        #expect(model.navigation.selectedSessionId == record.id)
        #expect(model.linkedSession == nil)
        // Retrying through the chooser uses the same failure receipt as a direct link.
        await model.chooseLinkedTask(wave: wave, task: task)
        #expect(model.taskOpening?.status == .failed)
        #expect(model.taskOpening?.reason == model.taskLinkReading.errorMessage)
        #expect(model.linkedSession == nil)
    }

    @Test(arguments: ["usable", "connection", "surface", "comparison", "closed", "hidden", "zoomed", "replaced"])
    func composedOpeningWaitsForNativeSessionAndComparison(outcome: String) async throws {
        let (model, record, url, query) = try openingFixture(outcome: outcome)
        await model.openTaskLink(url)
        let request = try #require(model.linkedSession)
        let workspace = SessionsWorkspace(identity: try #require(record.workspace?.identity))
        workspace.multiplexer.load(sessionId: record.id)
        workspace.multiplexer.show(.files(taskId: try #require(request.changesTask?.id)), focus: false)
        let store = SessionsStore(repoPath: try #require(model.repoPath), query: query)
        store.reconcile([record])
        let files = TaskFilesStore(issue: "LOO-427", cwd: "/fixture", query: query)
        files.selection = "retained-draft.rs"
        await model.connectLinkedOpening(request, store: store, workspace: workspace, files: files)
        if outcome != "connection", outcome != "comparison" {
            #expect(store.sessions.first?.state == .prepared)
            #expect(model.taskOpening?.status == .opening)
            let pane = workspace.multiplexer.focusedPane
            switch outcome {
            case "closed": workspace.multiplexer.close(pane.id)
            case "hidden": workspace.multiplexer.setCollapsed(paneId: pane.id, collapsed: true)
            case "zoomed": workspace.multiplexer.setZoom(pane.id, enabled: true)
            case "replaced": workspace.multiplexer.load(sessionId: "replacement")
            case "surface": store.recordPaneFailure(record.id, reason: "Native surface unavailable")
            default:
                // Exercise the native owner's callback, not a fabricated prepared=usable rule.
                store.recordPaneLive(record.id)
            }
        }
        try await waitForOpening(model)
        let reasons = ["connection": "Connection unavailable", "surface": "Native surface unavailable",
                       "comparison": "Comparison unavailable"]
        #expect(model.taskOpening?.status == (outcome == "usable" ? .usable : .failed))
        if outcome != "usable" {
            #expect(model.taskOpening?.reason == (reasons[outcome] ?? "Opening panes were hidden, closed or replaced."))
        }
        #expect(model.taskOpening?.url == url.absoluteString)
        #expect(files.selection == "retained-draft.rs")
    }

    @Test func missingSessionReadingDoesNotHidePaneClosure() async throws {
        let (model, record, url, query) = try openingFixture(outcome: "usable")
        await model.openTaskLink(url)
        let request = try #require(model.linkedSession)
        let workspace = SessionsWorkspace(identity: try #require(record.workspace?.identity))
        workspace.multiplexer.load(sessionId: record.id)
        workspace.multiplexer.show(.files(taskId: try #require(request.changesTask?.id)), focus: false)
        let store = SessionsStore(repoPath: try #require(model.repoPath), query: query)
        // An incomplete Session reading is not failure, but cannot hide a
        // definitive layout change while the request waits for that reading.
        await model.connectLinkedOpening(request, store: store, workspace: workspace, files: nil)
        #expect(model.taskOpening?.status == .opening)
        workspace.multiplexer.close(workspace.multiplexer.focusedPaneId)
        try await waitForOpening(model)
        #expect(model.taskOpening?.status == .failed)
        #expect(model.taskOpening?.reason == "Opening panes were hidden, closed or replaced.")
    }

    @Test(arguments: [false, true])
    func supersededOpeningCannotSettleRepeatedURL(cancel: Bool) async throws {
        let (model, record, url, query) = try openingFixture(outcome: "usable")
        await model.openTaskLink(url)
        let oldRequest = try #require(model.linkedSession)
        let workspace = SessionsWorkspace(identity: try #require(record.workspace?.identity))
        workspace.multiplexer.load(sessionId: record.id)
        workspace.multiplexer.show(.files(taskId: try #require(oldRequest.changesTask?.id)), focus: false)
        let oldStore = SessionsStore(repoPath: try #require(model.repoPath), query: query)
        oldStore.reconcile([record])
        await oldStore.select(record.id)
        await model.connectLinkedOpening(oldRequest, store: oldStore, workspace: workspace, files: nil)
        await model.openTaskLink(url)
        let newRequest = try #require(model.linkedSession)
        #expect(oldRequest.generation != newRequest.generation)
        let newStore = SessionsStore(repoPath: try #require(model.repoPath), query: query)
        newStore.reconcile([record])
        await newStore.select(record.id)
        let files = TaskFilesStore(issue: "LOO-427", cwd: "/fixture", query: query)
        await model.connectLinkedOpening(newRequest, store: newStore, workspace: workspace, files: files)
        if cancel { model.dismissTaskLink() }
        let before = model.taskOpening
        oldStore.recordPaneFailure(record.id, reason: "Late old failure")
        await model.connectLinkedOpening(oldRequest, store: oldStore, workspace: workspace, files: nil)
        #expect(model.taskOpening == before)
        newStore.recordPaneLive(record.id)
        try await waitForOpening(model)
        #expect(model.taskOpening?.status == (cancel ? .failed : .usable))
        #expect(model.taskOpening?.reason != "Late old failure")
    }

    @Test func canceledComparisonCannotSettleNewOpening() async throws {
        let barrier = LinkedDestinationBarrier()
        let (model, record, url, query) = try openingFixture(outcome: "usable", comparisonBarrier: barrier)
        await model.openTaskLink(url)
        let request = try #require(model.linkedSession)
        let workspace = SessionsWorkspace(identity: try #require(record.workspace?.identity))
        workspace.multiplexer.load(sessionId: record.id)
        workspace.multiplexer.show(.files(taskId: try #require(request.changesTask?.id)), focus: false)
        let store = SessionsStore(repoPath: try #require(model.repoPath), query: query)
        store.reconcile([record])
        await store.select(record.id)
        store.recordPaneLive(record.id)
        let files = TaskFilesStore(issue: "LOO-427", cwd: "/fixture", query: query)
        let reading = Task { await model.connectLinkedOpening(request, store: store, workspace: workspace, files: files) }
        while !(await barrier.contains("comparison")) { await Task.yield() }
        #expect(model.taskOpening?.status == .opening)
        await model.openTaskLink(url)
        let next = try #require(model.linkedSession)
        await barrier.release("comparison")
        await reading.value
        #expect(model.taskOpening?.status == .opening)
        #expect(model.linkedSession == next)
        model.dismissTaskLink()
        #expect(model.taskOpening?.status == .failed)
    }

    private func waitForOpening(_ model: WorkModel) async throws {
        let deadline = ContinuousClock.now + .seconds(3)
        while model.taskOpening?.status == .opening {
            guard ContinuousClock.now < deadline else { throw RegistryQueryError("Opening never settled") }
            try await Task.sleep(for: .milliseconds(5))
        }
    }

    private func openingFixture(outcome: String, comparisonBarrier: LinkedDestinationBarrier? = nil) throws -> (WorkModel, SessionRecord, URL, RegistryQuery) {
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(try fixture().utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        var record = try renameFixtureRecord("ready-session", title: "Review", work: .task(id: #require(task.runtime?.workId)))
        let identity = try #require(task.reference.workspace?.identity)
        record.workspace = try JSONDecoder().decode(SessionWorkspace.self, from: JSONSerialization.data(withJSONObject: [
            "machine_id": identity.machineId, "worktree": identity.worktree, "task_id": try #require(task.runtime?.workId)
        ]))
        var sessionJSON = try JSONSerialization.jsonObject(with: JSONEncoder().encode(record)) as! [String: Any]
        sessionJSON["actions"] = sessionActionFixture(state: "closed")
        record = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: sessionJSON))
        let encoded = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let fixtureURL = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto/task_files.json")
        let root = try JSONSerialization.jsonObject(with: Data(contentsOf: fixtureURL)) as! [String: Any]
        let changes = String(decoding: try JSONSerialization.data(withJSONObject: #require(root["changes"])), as: UTF8.self)
        let query = RegistryQuery { args, _ in
            if args.starts(with: ["session", "list"]) { return #"{"entries":[\#(encoded)],"next":null}"# }
            if args.starts(with: ["session", "connect"]) {
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
        return (model, record, try #require(TaskLink(issue: task.task.identifier, repo: wave.wave.repo, session: record.id, diff: true).url), query)
    }

    @Test func delayedPrimarySessionPreparationDoesNotOpenChangesInAnotherTask() async throws {
        let data = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let record = try renameFixtureRecord("prepared-session", title: "Review", work: .task(id: #require(task.runtime?.workId)))
        let encoded = String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        let barrier = LinkedDestinationBarrier()
        let model = WorkModel(query: RegistryQuery { args, _ in
            guard args.starts(with: ["session", "ensure"]) else { throw RegistryQueryError("Canceled preparation must not read or open another Task") }
            await barrier.wait("ensure")
            return encoded
        }, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        let url = try #require(TaskLink(issue: task.task.identifier, repo: wave.wave.repo, diff: true).url)
        let opening = Task { await model.openTaskLink(url) }
        while !(await barrier.contains("ensure")) { await Task.yield() }
        model.select(.wave(id: wave.wave.id))
        await barrier.release("ensure")
        await opening.value
        #expect(model.selection == .wave(id: wave.wave.id))
        #expect(model.linkedSession == nil)
    }

    @Test func paletteBindingTargetsOnlyTheSelectedUnassignedSession() async throws {
        let scopes: [WorkReference?] = [nil, .wave(id: "wave-product"), .task(id: "task-bound")]
        for work in scopes {
            let record = try renameFixtureRecord("binding-session", title: "Conversation", work: work)
            let sessions = String(decoding: try JSONEncoder().encode([record]), as: UTF8.self)
            let model = WorkModel(query: RegistryQuery { args, _ in
                if args.first == "session" { return #"{"entries":\#(sessions),"next":null}"# }
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

    @Test func taskLinkParsesOneDecodedIdentifier() throws {
        let link = try TaskLink(url: #require(URL(string: "loopflow://task/LOO-303?repo=%2Fsrc%2Fspace%20here")))
        #expect(link.issue == "LOO-303")
        #expect(link.repo == "/src/space here")
        let sessionLink = try TaskLink(url: #require(URL(string: "loopflow://task/LOO-303?session=review%201&repo=%2Fsrc")))
        #expect(sessionLink.session == "review 1")
        #expect(sessionLink.repo == "/src")
        for value in ["loopflow://task/A?session=", "loopflow://task/A?session=x&session=y"] {
            #expect(throws: (any Error).self) { try TaskLink(url: #require(URL(string: value))) }
        }
        for value in ["loopflow://task/", "loopflow://task/A/B", "loopflow://task/A%2FB", "loopflow://task/A%0AB", "loopflow://task/A?repo=", "loopflow://task/A?repo=x&repo=y", "loopflow://task/A#node"] {
            let url = try #require(URL(string: value))
            #expect(throws: (any Error).self) { try TaskLink(url: url) }
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
        let model = WorkModel(query: RegistryQuery { args, cwd in
            if args.contains("--task") { return exact }
            if args.first == "session", cwd == wave.wave.repo {
                if args.contains("--after") { return #"{"entries":[\#(encoded),\#(otherEncoded)],"next":null}"# }
                return #"{"entries":[],"next":"page-two"}"#
            }
            throw RegistryQueryError("No mutation permitted")
        })
        var url = try #require(URLComponents(string: "loopflow://task/\(task.task.identifier)"))
        url.queryItems = [URLQueryItem(name: "repo", value: wave.wave.repo), URLQueryItem(name: "session", value: record.id)]
        await model.openTaskLink(try #require(url.url))
        #expect(model.navigation.selectedSessionId == record.id)
        #expect(model.navigation.content == .terminals)
        #expect(model.linkedSession?.record.id == record.id)
        #expect(!model.showsTaskLink)

        for session in ["missing", unrelated.id] {
            url.queryItems = [URLQueryItem(name: "session", value: session)]
            await model.openTaskLink(try #require(url.url))
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
            if args.prefix(2) == ["wave", "show"] { return #"{"generated_at":"2026-09-26T00:00:00Z","waves":[]}"# }
            if args.first == "session" { return #"{"entries":[],"next":null}"# }
            if args.first == "wave" { return "[]" }
            throw RegistryQueryError("No mutation permitted")
        }
        let destination = WorkModel(query: exactQuery)
        await destination.openTaskLink(try #require(URL(string: "loopflow://task/\(task.task.identifier)")))
        #expect(destination.selection == .task(id: task.id))
        #expect(destination.navigation.content == .details)
        #expect(destination.navigation.selectedSessionId == nil)
        await destination.refresh()
        #expect(destination.selection == .task(id: task.id))
        #expect(destination.task(id: task.id)?.task.task.name == task.task.name)
        #expect(destination.visibleRoadmaps.isEmpty)
        #expect(destination.breadcrumb?.task?.task.id == task.id)
        let registry = SessionsWorkspaceRegistry()
        let workspace = registry.workspace(for: try #require(task.reference.workspace?.identity))
        workspace.multiplexer.newShell(command: ["local-server"])
        let layout = workspace.multiplexer.layout
        let document = workspace.files(taskId: task.id, issue: task.task.identifier, cwd: wave.wave.repo,
                                       query: exactQuery).document("note.txt")
        document.editor.string = "unsaved draft"
        let retained = SessionsContentView(
            model: destination, repoPath: wave.wave.repo,
            workspaces: registry, machineId: "local", query: exactQuery
        )
        #expect(throws: Never.self) {
            try retained.inspect().find(viewWithAccessibilityIdentifier: "task-worktree-location")
        }
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
        let exact = try oneTask(data, taskId: task.id)
        // Keep the Wave, but remove the historical Task from the current plan.
        let current = try oneTask(data, taskId: "absent")
        let model = WorkModel(query: RegistryQuery { args, _ in
            if args.contains("--task") { return exact }
            if args.prefix(2) == ["wave", "show"] { return current }
            if args.first == "session" { return #"{"entries":[],"next":null}"# }
            if args.first == "wave" { return "[]" }
            throw RegistryQueryError("No mutation permitted")
        }, repoPath: wave.wave.repo)
        await model.openTaskLink(try #require(URL(string: "loopflow://task/\(task.task.identifier)")))
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
            let model = WorkModel(query: RegistryQuery { args, _ in
                if args.contains("--task") {
                    if response == "failure" { throw RegistryQueryError("Machine unavailable") }
                    return response
                }
                if args.prefix(2) == ["wave", "show"] { return current }
                if args.first == "session" { return #"{"entries":[],"next":null}"# }
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
        let model = WorkModel(query: RegistryQuery { args, _ in
            if args.prefix(2) == ["wave", "show"] { return data }
            if args.first == "session" { return #"{"entries":\#(sessions),"next":null}"# }
            if args.first == "wave" { return "[]" }
            throw RegistryQueryError("Unavailable")
        }, repoPath: "/src/loopflow")
        await model.refresh()
        let tasks = model.visibleRoadmaps.flatMap { $0.tasks.items }
        let task = try #require(tasks.first)
        #expect(model.searchDestinations(task.task.identifier).first?.id == .task(task.id))
        #expect(model.paletteRows.filter { if case .task = $0.id { true } else { false } }.count == tasks.count)
        let matchingSessions = ([1] + Array(10...19)).map { WorkDestination.session(String($0)) }
        model.remember(.session("19"))
        #expect(model.searchDestinations(" session 1 ").map(\.id) == matchingSessions)
        model.remember(.task(task.id))
        #expect(model.searchDestinations("").first?.id == .task(task.id))
        for index in 0..<30 { model.remember(.session("\(index)")) }
        #expect(model.navigation.recentDestinations.count == 20)
    }

    @Test func historicalTaskWithoutPlanningHasNoDeadEndFlowAction() async throws {
        var snapshot = try #require(JSONSerialization.jsonObject(with: Data(fixture().utf8)) as? [String: Any])
        var waves = try #require(snapshot["waves"] as? [[String: Any]])
        var unavailable = try #require(waves[0]["unavailable_tasks"] as? [[String: Any]])
        unavailable[0]["task_id"] = "issue-available"
        waves[0]["unavailable_tasks"] = unavailable
        snapshot["waves"] = waves
        let data = String(decoding: try JSONSerialization.data(withJSONObject: snapshot), as: UTF8.self)
        let exact = try oneTask(data, taskId: "issue-available")
        let model = WorkModel(query: RegistryQuery { _, _ in exact })
        await model.openTaskLink(try #require(URL(string: "loopflow://task/PRD-52")))
        #expect(model.selection == .task(id: "issue-available"))
        #expect(model.paletteRows.contains { $0.id == .flowLog("issue-available") })
    }

    @Test func ambiguousAndUnavailableLinksPreserveTheWorkspace() async throws {
        let data = try fixture()
        for response in [data, #"{"generated_at":"2026-09-26T00:00:00Z","waves":[]}"#, "transport-error"] {
            let model = WorkModel(query: RegistryQuery { args, _ in
                if args.contains("--task") {
                    if response == "transport-error" { throw RegistryQueryError("Machine unavailable") }
                    return response
                }
                if args.prefix(2) == ["wave", "show"] { return data }
                if args.first == "session" { return #"{"entries":[],"next":null}"# }
                if args.first == "wave" { return "[]" }
                throw RegistryQueryError("No mutation permitted")
            }, repoPath: "/src/loopflow")
            await model.refresh()
            let wave = try #require(model.visibleRoadmaps.first)
            model.select(.wave(id: wave.wave.id))
            await model.openTaskLink(try #require(URL(string: "loopflow://task/LOO-303")))
            #expect(model.selection == .wave(id: wave.wave.id))
            #expect(model.showsTaskLink)
            if response == "transport-error" {
                #expect(model.taskLinkReading.errorMessage == "Machine unavailable")
            } else {
                #expect(model.taskLinkReading.errorMessage == nil)
            }
        }
    }

    @Test(arguments: [false, true])
    func scopedTaskLinkOpensItsMatchWithAnotherWaveUnavailable(scoped: Bool) async throws {
        let data = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let exact = try oneTask(data, taskId: task.id)
        var result = try #require(JSONSerialization.jsonObject(with: Data(exact.utf8)) as? [String: Any])
        var waves = try #require(result["waves"] as? [[String: Any]])
        let other = try #require(waves.indices.first { index in
            let tasks = waves[index]["tasks"] as? [String: Any]
            return (tasks?["items"] as? [Any])?.isEmpty == true
        })
        waves[other]["tasks"] = ["state": "unavailable", "reason": "Chapter unavailable"]
        result["waves"] = waves
        let response = String(decoding: try JSONSerialization.data(withJSONObject: result), as: UTF8.self)
        let model = WorkModel(query: RegistryQuery { _, _ in response })
        var link = try #require(URLComponents(string: "loopflow://task/\(task.task.identifier)"))
        if scoped { link.queryItems = [URLQueryItem(name: "repo", value: wave.wave.repo)] }
        await model.openTaskLink(try #require(link.url))
        #expect(model.selection == (scoped ? .task(id: task.id) : nil))
        #expect(model.showsTaskLink == !scoped)
    }

    @Test func inspectionForwardsTaskControlIndependentlyOfSessionFailure() throws {
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(fixture().utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let model = WorkModel(query: RegistryQuery { _, _ in
            throw RegistryQueryError("Inspection must not read or mutate Work")
        }, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        model.openTaskDestination(wave: wave, task: task)
        model.navigation.selectedSessionId = "not-read-yet"
        let workspaces = SessionsWorkspaceRegistry()
        let report = model.inspectDesktop(repository: "plan", window: UUID(), workspaces: workspaces)
        #expect(report.reading == "loading")
        #expect(report.supportedOperations.contains("list"))
        #expect(report.supportedOperations.contains("read"))
        #expect(!report.supportedOperations.contains("inspect"))
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
        let model = WorkModel(query: RegistryQuery { _, _ in
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
        let model = WorkModel(query: RegistryQuery { _, _ in
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

        let empty = RoadmapSnapshot(generatedAt: "2026-10-08T21:00:00Z", waves: [])
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
        let model = WorkModel(query: RegistryQuery { args, _ in
            guard args.first == "session" else { throw RegistryQueryError("No other reads permitted") }
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
        let model = WorkModel(query: RegistryQuery { args, _ in
            guard args.first == "session" else { throw RegistryQueryError("No other reads permitted") }
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
        let model = WorkModel(query: RegistryQuery { _, _ in
            throw RegistryQueryError("No Session preparation is required to show a Task page")
        }, repoPath: wave.wave.repo)
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        let url = try #require(TaskLink(issue: task.task.identifier, repo: wave.wave.repo).url)
        await model.openTaskLink(url)
        let old = try #require(model.linkedTaskPage)
        #expect(model.taskOpening?.status == .opening)
        #expect(model.linkedSession == nil)
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
        var received: [TaskLink] = []
        router.register(window, repository: workspace.id, focus: {}, inspect: windowInspection,
            controlPane: { _ in }, readText: { _ in throw RegistryQueryError("No terminal") }) { url in
            do { received.append(try TaskLink(url: url)) } catch { Issue.record(error) }
        }
        for machine in ["local-owner", "peer-owner"] {
            let link = TaskLink(issue: "task-\(machine)", repo: path, session: "session-\(machine)", diff: true,
                                machine: machine, repository: "plan")
            let url = try #require(link.url)
            _ = try await router.openRepository(path: path, link: url, query: query) { _ in
                Issue.record("Execution Machine must not create another repository window")
            }
        }
        while received.count < 2 { await Task.yield() }
        #expect(received.map(\.machine) == ["local-owner", "peer-owner"])
        #expect(received.allSatisfy { $0.repository == "plan" && $0.diff })
        #expect(router.inspect().windows.map(\.window) == [window.uuidString])
    }

    @Test func plainRepositoryWaitsForRegistrationAndReusesTheWindow() async throws {
        let router = WorkLinkRouter()
        let query = RegistryQuery { _, _ in #"{"id":"plan","locators":["plan"]}"# }
        let workspace = try await router.openRepository(path: repositoryFixturePath(), link: nil, query: query) { _ in }
        #expect(router.inspect().windows.isEmpty)
        #expect(router.inspect().openings.first?.status == .opening)
        let id = UUID()
        router.register(id, repository: workspace.id, openingRequests: router.workspaceRequests(workspace.id), focus: {}, inspect: windowInspection,
                        controlPane: { _ in }, readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
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
                        controlPane: { _ in }, readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
        #expect(router.inspect().openings.first?.status == .opening)
        router.register(UUID(), repository: current.id, openingRequests: router.workspaceRequests(current.id), focus: {}, inspect: windowInspection,
                        controlPane: { _ in }, readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
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
                        controlPane: { _ in }, readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
        #expect(router.inspect().openings.first?.status == .opening)
        router.remove(stale, repository: workspace.id)
        _ = try await router.resolveWorkspace(workspace, query: query)
        router.register(UUID(), repository: workspace.id, openingRequests: router.workspaceRequests(workspace.id), focus: {}, inspect: windowInspection,
                        controlPane: { _ in }, readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
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
                        controlPane: { _ in }, readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
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
                        controlPane: { _ in }, readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
        #expect(router.inspect().openings == receipt)
    }

    @Test func repositoryAssociationRetainsDraftsPanesAndInFlightDelivery() async throws {
        let router = WorkLinkRouter(), barrier = LinkedDestinationBarrier()
        let path = repositoryFixturePath()
        let before = RegistryQuery { _, _ in #"{"id":"old-plan","locators":["old-plan"]}"# }
        let after = RegistryQuery { _, _ in #"{"id":"plan","locators":["old-plan","plan"]}"# }
        let first = try #require(TaskLink(issue: "FIRST", repo: path).url)
        let second = try #require(TaskLink(issue: "SECOND", repo: path).url)
        let scene = try await router.openRepository(path: path, link: first, query: before) { _ in }
        let deliveredFirst = try #require(TaskLink(issue: "FIRST", repo: scene.path).url)
        let deliveredSecond = try #require(TaskLink(issue: "SECOND", repo: scene.path).url)
        let registry = SessionsWorkspaceRegistry(localMachineId: "fixture-machine")
        let identity = WorkspaceIdentity(machineId: "fixture-machine", worktree: path)
        let retained = registry.workspace(for: identity), store = retained.multiplexer
        store.newShell(command: ["retained-command"])
        let pane = store.focusedPane
        let document = retained.files(taskId: "task", issue: "TASK", cwd: path).document("note.txt")
        document.editor.string = "unfinished draft"
        document.editor.setSelectedRange(NSRange(location: 2, length: 4))
        let layout = store.layout, focus = store.focusedPaneId
        #if canImport(GhosttyKit)
        let terminal = TerminalIdentity.shell(pane.id, machineId: identity.machineId)
        let view = registry.surfaces.view(for: terminal)
        defer { registry.surfaces.release(terminal) }
        #endif
        let window = UUID()
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("No Work reads") })
        var received: [URL] = []
        router.register(window, repository: scene.id, openingRequests: router.workspaceRequests(scene.id), focus: {},
            inspect: { repository, incarnation in
                DesktopWindowInspection(repository: repository, window: incarnation.uuidString, path: path,
                    selectionKind: nil, selectionId: nil, reading: "current", reason: nil,
                    task: nil, session: nil, supportedOperations: ["hide", "restore"],
                    workspaces: registry.inspect(), layouts: registry.inspectLayouts(), opening: nil)
            }, controlPane: { try registry.controlPane($0, model: model) },
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
        #expect(registry.workspace(for: identity) === retained)
        #if canImport(GhosttyKit)
        #expect(registry.surfaces.view(for: terminal) === view)
        #expect(registry.surfaces.programStatus(for: terminal) === view.programStatus)
        #endif
        #expect(store.layout == layout)
        #expect(store.focusedPaneId == focus)
        #expect(document.editor.string == "unfinished draft")
        #expect(document.editor.selectedRange() == NSRange(location: 2, length: 4))
        let stale = DesktopPaneTarget(repository: "old-plan", window: window.uuidString,
            machineId: identity.machineId, worktree: path, pane: pane.id, incarnation: pane.incarnation)
        #expect(throws: RegistryQueryError.self) {
            try router.controlPane(.init(target: stale, action: .hide))
        }
        let current = DesktopPaneTarget(repository: "plan", window: report.windows[0].window,
            machineId: identity.machineId, worktree: path, pane: pane.id, incarnation: pane.incarnation)
        _ = try router.controlPane(.init(target: current, action: .hide))
        _ = try router.controlPane(.init(target: current, action: .restore))
        #expect(store.layout == layout)
        await barrier.release("delivery")
        while received.count < 2 { await Task.yield() }
        #expect(received == [deliveredFirst, deliveredSecond])
        #expect(document.editor.string == "unfinished draft")
        _ = try await router.openRepository(path: path, link: nil, query: RegistryQuery { _, _ in
            #"{"id":"old-plan","locators":["old-plan","plan"]}"#
        }) { _ in Issue.record("Binding back must also reuse the window") }
        #expect(router.inspect().windows.first?.repository == "old-plan")
        #expect(throws: RegistryQueryError.self) {
            try router.controlPane(.init(target: stale, action: .hide))
        }
        #expect(throws: RegistryQueryError.self) {
            try router.controlPane(.init(target: current, action: .hide))
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
            inspect: windowInspection, controlPane: { _ in },
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
                controlPane: { _ in }, readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
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
                controlPane: { _ in }, readText: { _ in throw RegistryQueryError("No terminal") }) { _ in }
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
        try #require(RepoScanner().mainRepository(alias) != nil)
        let window = UUID()
        var received: [URL] = []
        let register: (RepositoryWorkspace) -> Void = { scene in
            router.register(window, repository: scene.id, openingRequests: router.workspaceRequests(scene.id),
                focus: {}, inspect: windowInspection,
                controlPane: { _ in }, readText: { _ in throw RegistryQueryError("No terminal") }) { received.append($0) }
        }
        if alreadyOpen {
            _ = try await router.openRepository(path: path, link: nil,
                query: RegistryQuery { _, _ in #"{"id":"old-plan","locators":["old-plan"]}"# }, openWindow: register)
        }
        let link = try #require(TaskLink(issue: "PENDING", repo: alias.path).url)
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
        #expect(received == [TaskLink(issue: "PENDING", repo: scene.path).url])
        #expect(router.inspect().openings.allSatisfy { $0.status == .usable })
    }

    private func repositoryFixturePath() -> String {
        URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().path
    }

    private func windowInspection(_ repository: String, _ id: UUID) -> DesktopWindowInspection {
        DesktopWindowInspection(repository: repository, window: id.uuidString, path: nil,
            selectionKind: nil, selectionId: nil, reading: "loading", reason: nil,
            task: nil, session: nil, supportedOperations: ["list"], workspaces: [], layouts: [], opening: nil)
    }

    @Test func inspectionIncludesOnlyRegisteredWindowIncarnationsWithoutFocusing() {
        let router = WorkLinkRouter()
        let first = UUID(), replacement = UUID()
        var focused = false
        router.register(first, repository: "plan", focus: { focused = true }, inspect: windowInspection, controlPane: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { _ in }
        #expect(router.inspect().windows.map(\.window) == [first.uuidString])
        router.register(replacement, repository: "plan", focus: { focused = true }, inspect: windowInspection, controlPane: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { _ in }
        router.remove(first, repository: "plan")
        #expect(router.inspect().windows.map(\.window) == [replacement.uuidString])
        router.remove(replacement, repository: "plan")
        #expect(router.inspect().windows.isEmpty)
        #expect(!focused)
    }

    @Test func coldRepositoriesRegisterInReverseOrderWithoutLosingDestinations() async throws {
        let router = WorkLinkRouter()
        let a = try #require(URL(string: "loopflow://task/A"))
        let a2 = try #require(URL(string: "loopflow://task/A2"))
        let b = try #require(URL(string: "loopflow://task/B"))
        #expect(!router.deliver(a, repository: "plan-a"))
        #expect(!router.deliver(b, repository: "plan-b"))
        #expect(!router.deliver(a2, repository: "plan-a"))
        var first: [URL] = []
        var second: [URL] = []
        router.register(UUID(), repository: "plan-b", focus: {}, inspect: windowInspection, controlPane: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { second.append($0) }
        while second.isEmpty { await Task.yield() }
        #expect(second == [b])
        #expect(first.isEmpty)
        router.register(UUID(), repository: "plan-a", focus: {}, inspect: windowInspection, controlPane: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { first.append($0) }
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
        var received: [URL] = []
        let url = try #require(URL(string: "loopflow://task/A"))
        router.register(UUID(), repository: a.id, focus: { focused.append("retained") }, inspect: windowInspection, controlPane: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { received.append($0) }
        router.register(UUID(), repository: "other", focus: { focused.append("other") }, inspect: windowInspection, controlPane: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { _ in
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
        router.register(old, repository: "plan", focus: {}, inspect: windowInspection, controlPane: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { _ in }
        var received: [URL] = []
        router.register(replacement, repository: "plan", focus: {}, inspect: windowInspection, controlPane: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { received.append($0) }
        router.remove(old, repository: "plan")
        let url = try #require(URL(string: "loopflow://task/A"))
        #expect(router.deliver(url, repository: "plan"))
        while received.isEmpty { await Task.yield() }
        #expect(received == [url])
        router.remove(replacement, repository: "plan")
        #expect(!router.deliver(url, repository: "plan"))
    }

    @Test func delayedDeliveryKeepsOneQueueAcrossReceiverReplacement() async throws {
        let router = WorkLinkRouter()
        let first = try #require(URL(string: "loopflow://task/first"))
        let second = try #require(URL(string: "loopflow://task/second"))
        let third = try #require(URL(string: "loopflow://task/third"))
        let other = try #require(URL(string: "loopflow://task/other"))
        let old = UUID(), replacement = UUID()
        let barrier = LinkedDestinationBarrier()
        var delivered: [URL] = []
        router.register(old, repository: "plan", focus: {}, inspect: windowInspection, controlPane: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { link in
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
        router.register(replacement, repository: "plan", focus: {}, inspect: windowInspection, controlPane: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { delivered.append($0) }
        router.remove(old, repository: "plan")

        // A slow repository never blocks another repository's opening.
        router.register(UUID(), repository: "other", focus: {}, inspect: windowInspection, controlPane: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { delivered.append($0) }
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
        let first = try #require(URL(string: "loopflow://task/first"))
        let second = try #require(URL(string: "loopflow://task/second"))
        let barrier = LinkedDestinationBarrier()
        var delivered: [URL] = []
        router.register(id, repository: "plan", focus: {}, inspect: windowInspection, controlPane: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { _ in await barrier.wait("old") }
        router.deliver(first, repository: "plan")
        while !(await barrier.contains("old")) { await Task.yield() }
        router.remove(id, repository: "plan")
        router.register(id, repository: "plan", focus: {}, inspect: windowInspection, controlPane: { _ in throw RegistryQueryError("No panes in this fixture") }, readText: { _ in throw RegistryQueryError("No panes in this fixture") }) { link in
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
        let model = WorkModel(query: RegistryQuery { _, _ in
            await barrier.wait("read")
            if fails { throw RegistryQueryError("Delayed failure") }
            return exact
        }, repoPath: "/origin")
        model.navigation.selectedSessionId = "retained-session"
        let link = try #require(URL(string: "loopflow://task/\(task.task.identifier)"))
        let opening = Task { await model.openTaskLink(link) }
        while !(await barrier.contains("read")) { await Task.yield() }
        opening.cancel()
        await barrier.release("read")
        await opening.value
        #expect(model.repoPath == "/origin")
        #expect(model.selection == nil)
        #expect(model.navigation.selectedSessionId == "retained-session")
        #expect(model.linkedSession == nil)
        #expect(!model.showsTaskLink)
    }

    @Test func repositoryNavigationLeavesTheOriginSelectionAndDraftWorkspaceAlone() async throws {
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(try fixture().utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let model = WorkModel(query: RegistryQuery { _, _ in "" }, repoPath: "/origin")
        model.navigation.selectedSessionId = "retained-session"
        var requests: [(String, URL?)] = []
        model.openRepository = { requests.append(($0, $1)) }
        model.setRepoPath("/other")
        model.openTaskDestination(wave: wave, task: task)
        #expect(model.repoPath == "/origin")
        #expect(model.navigation.selectedSessionId == "retained-session")
        #expect(requests.map { $0.0 } == ["/other", wave.wave.repo])
        #expect(try TaskLink(url: #require(requests.last?.1)).issue == task.task.identifier)
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
            if args.prefix(2) == ["wave", "show"] { return data }
            if args.first == "session" { return #"{"entries":[],"next":null}"# }
            if args.first == "wave" { return "[]" }
            throw RegistryQueryError("No mutation permitted")
        }
        let model = WorkModel(query: query, repoPath: wave.wave.repo)
        await model.refresh()
        model.select(.wave(id: wave.wave.id))
        let firstURL = try #require(URL(string: "loopflow://task/\(first.task.identifier)"))
        let secondURL = try #require(URL(string: "loopflow://task/\(second.task.identifier)"))
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
        var waves = try #require(snapshot["waves"] as? [[String: Any]])
        for index in waves.indices {
            var evidence = try #require(waves[index]["tasks"] as? [String: Any])
            let items = evidence["items"] as? [[String: Any]] ?? []
            evidence["state"] = "ok"
            evidence["truncated"] = false
            evidence["items"] = items.filter { ($0["task"] as? [String: Any])?["id"] as? String == taskId }
            waves[index]["tasks"] = evidence
        }
        snapshot["waves"] = waves
        return String(decoding: try JSONSerialization.data(withJSONObject: snapshot), as: UTF8.self)
    }
}

private actor LinkedDestinationBarrier {
    private var pending: [String: CheckedContinuation<Void, Never>] = [:]
    func contains(_ key: String) -> Bool { pending[key] != nil }
    func wait(_ key: String) async {
        await withCheckedContinuation { pending[key] = $0 }
    }
    func release(_ key: String) { pending.removeValue(forKey: key)?.resume() }
}

#endif

private actor DestinationReadCounter {
    private var count = 0
    func next() -> Int { count += 1; return count }
}
