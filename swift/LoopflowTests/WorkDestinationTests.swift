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
        #expect(model.linkedSession?.id == record.id)
        #expect(model.navigation.selectedSessionId == record.id)
        #expect(model.taskLinkReading.errorMessage == nil)
        // Reopening from retained evidence must preserve the conversation too.
        await model.openTaskLink(try #require(url.url))
        #expect(model.navigation.selectedSessionId == record.id)
        #expect(model.sessions.value?.map(\.id) == [record.id, other.id])
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
        #expect(model.linkedSession?.id == record.id)
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
            if args.first == "roadmap" { return #"{"generated_at":"2026-09-26T00:00:00Z","waves":[]}"# }
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
            if args.first == "roadmap" { return current }
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
                if args.first == "roadmap" { return current }
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
            if args.first == "roadmap" { return data }
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
                if args.first == "roadmap" { return data }
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

    @Test func coldRepositoriesRegisterInReverseOrderWithoutLosingDestinations() throws {
        let router = WorkLinkRouter()
        let a = try #require(URL(string: "loopflow://task/A"))
        let a2 = try #require(URL(string: "loopflow://task/A2"))
        let b = try #require(URL(string: "loopflow://task/B"))
        #expect(!router.deliver(a, repository: "plan-a"))
        #expect(!router.deliver(b, repository: "plan-b"))
        #expect(!router.deliver(a2, repository: "plan-a"))
        var first: [URL] = []
        var second: [URL] = []
        router.register(UUID(), repository: "plan-b", focus: {}) { second += $0 }
        #expect(second == [b])
        #expect(first.isEmpty)
        router.register(UUID(), repository: "plan-a", focus: {}) { first += $0 }
        #expect(first == [a, a2])
        #expect(second == [b])
    }

    @Test func samePlanReusesItsRetainedWorkspaceRegardlessOfFocusOrLocator() throws {
        let a = RepositoryWorkspace(id: "plan", path: "/machine-a/repo")
        let b = RepositoryWorkspace(id: "plan", path: "/machine-b/repo")
        #expect(Set([a, b]).count == 1)
        #expect(a != RepositoryWorkspace(id: "another-plan", path: a.path))
        let router = WorkLinkRouter()
        var focused: [String] = []
        var received: [URL] = []
        let url = try #require(URL(string: "loopflow://task/A"))
        router.register(UUID(), repository: a.id, focus: { focused.append("retained") }) { received += $0 }
        router.register(UUID(), repository: "other", focus: { focused.append("other") }) { _ in
            Issue.record("A repository must never receive another repository's link")
        }
        #expect(router.deliver(url, repository: b.id))
        #expect(router.deliver(nil, repository: a.id))
        #expect(focused == ["retained", "retained"])
        #expect(received == [url])
    }

    @Test func lateWindowRemovalDoesNotRemoveItsReplacement() throws {
        let router = WorkLinkRouter()
        let old = UUID(), replacement = UUID()
        router.register(old, repository: "plan", focus: {}) { _ in }
        var received: [URL] = []
        router.register(replacement, repository: "plan", focus: {}) { received += $0 }
        router.remove(old, repository: "plan")
        let url = try #require(URL(string: "loopflow://task/A"))
        #expect(router.deliver(url, repository: "plan"))
        #expect(received == [url])
        router.remove(replacement, repository: "plan")
        #expect(!router.deliver(url, repository: "plan"))
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
            if args.first == "roadmap" { return data }
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
