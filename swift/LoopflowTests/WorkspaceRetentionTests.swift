#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Application workspace retention")
@MainActor
struct WorkspaceRetentionTests {
    @Test("Recent unfinished Tasks stay warm; Close preserves history and releases retention")
    func hostRetentionFollowsTheNativeWorkspace() {
        let registry = SessionsWorkspaceRegistry(localMachineId: "local")
        let first = WorkWorkspaceKey(repository: "repo", kind: .task, id: "first")
        let second = WorkWorkspaceKey(repository: "repo", kind: .task, id: "second")
        let a = registry.open(first, title: "First", location: nil)
        a.multiplexer.load(sessionId: "history")
        #expect(a.hostUse == "retained")
        #expect(!a.nativeSettled)
        let b = registry.open(second, title: "Second", location: nil)
        #expect(a.hostUse == "retained")
        #expect(!a.nativeSettled)
        #expect(b.hostUse == "retained")
        registry.closePresentation()
        #expect(a.hostUse == "closed")
        #expect(b.hostUse == "closed")
        #expect(a.multiplexer.sessionPane?.sessionId == "history")
        #expect(registry.retained(first) === a)
        _ = registry.open(first, title: "First", location: nil)
        #expect(a.hostUse == "retained")
        #expect(!a.nativeSettled)
    }

    @Test("Warm visits are bounded; completed departure and Close release without refresh reopening")
    func boundedWarmVisits() {
        let registry = SessionsWorkspaceRegistry(localMachineId: "local")
        let limit = SessionsWorkspaceRegistry.warmConversationLimit
        let keys = (0..<limit + 2).map { WorkWorkspaceKey(repository: "repo", kind: .task, id: "task-\($0)") }
        for key in keys { registry.open(key, title: key.id, location: nil) }
        #expect(registry.recentTasks.count == limit)
        #expect(registry.retained(keys[0])?.hostUse == "away")
        let latest = registry.retained(keys[limit + 1])!
        registry.updateCompletion(keys[limit + 1], completed: true)
        #expect(latest.hostUse == "retained")
        registry.open(keys[0], title: "First", location: nil)
        #expect(latest.hostUse == "away")
        let first = registry.retained(keys[0])!
        first.multiplexer.load(sessionId: "conversation")
        first.closePane(first.multiplexer.sessionPane!.id)
        #expect(first.conversationClosed)
        #expect(!registry.recentTasks.contains(keys[0]))
        #expect(first.hostUse == "away")
        registry.open(keys[0], title: "First", location: nil)
        #expect(!first.conversationClosed)
        #expect(first.hostUse == "retained")
    }

    @Test("The warm-conversation bound spans repository windows without sharing surfaces")
    func warmthIsApplicationWide() {
        let application = RepositoryWorkspaces(localMachineId: "local")
        let first = application.registry(for: "first")
        let second = application.registry(for: "second")
        let limit = SessionsWorkspaceRegistry.warmConversationLimit
        for index in 0..<limit {
            first.open(WorkWorkspaceKey(repository: "first", kind: .task, id: "a-\(index)"), title: "A", location: nil)
        }
        for index in 0..<limit {
            second.open(WorkWorkspaceKey(repository: "second", kind: .task, id: "b-\(index)"), title: "B", location: nil)
        }
        #expect(first.recentTasks.count + second.recentTasks.count == limit)
        #expect(first.recentTasks.count == 1) // Its visible pane stays retained.
        #expect(first.surfaces !== second.surfaces)
    }

    @Test("Restart restores bounded warm intent and preserves explicit Close")
    func restoresWarmIntent() {
        let repository = "repo_" + UUID().uuidString
        let registry = SessionsWorkspaceRegistry(localMachineId: "local")
        registry.loadWarmIntent(repository: repository)
        let limit = SessionsWorkspaceRegistry.warmConversationLimit
        let keys = (0..<limit + 2).map { WorkWorkspaceKey(repository: repository, kind: .task, id: "saved-\($0)") }
        for key in keys { registry.open(key, title: key.id, location: nil) }
        let last = registry.retained(keys[limit + 1])!
        last.multiplexer.load(sessionId: "closed")
        last.closePane(last.multiplexer.sessionPane!.id)
        registry.prepareForQuit()
        registry.closePresentation() // AppKit window teardown must not erase warm intent.
        let restored = SessionsWorkspaceRegistry(localMachineId: "local")
        restored.loadWarmIntent(repository: repository)
        #expect(restored.restoreCandidates.count == limit - 1)
        #expect(!restored.restoreCandidates.contains(keys[limit + 1]))
        #expect(restored.recentTasks.isEmpty) // Saved intent alone acquires no hold.
        let closed = restored.open(keys[limit + 1], title: "Closed", location: nil, deliberate: false)
        #expect(closed.conversationClosed)
        #expect(closed.hostUse == "away")
        let interruptedRestore = SessionsWorkspaceRegistry(localMachineId: "local")
        interruptedRestore.loadWarmIntent(repository: repository)
        #expect(interruptedRestore.restoreCandidates.count == limit - 1) // No fresh read has settled these yet.
        restored.open(keys[limit + 1], title: "Closed", location: nil)
        #expect(!closed.conversationClosed)
        #expect(closed.hostUse == "retained")
        UserDefaults.standard.removeObject(forKey: "warm-conversations-local-\(repository.lowercased())")
    }

    @Test("A working Task shown as a tile stays warm after the Dashboard leaves")
    func tiledWorkingTaskStaysWarm() {
        let registry = SessionsWorkspaceRegistry(localMachineId: "local")
        let repository = WorkWorkspaceKey(repository: "repo", kind: .repository, id: "repo")
        let tiled = WorkWorkspaceKey(repository: "repo", kind: .task, id: "tiled")
        let location = WorkspaceIdentity(machineId: "local", worktree: "/tmp/task")
        _ = registry.workspace(for: repository, title: "repo", location: nil)
        let tile = registry.workspace(for: tiled, title: "Tiled", location: location)
        registry.present(open: [repository, tiled])
        #expect(tile.hostUse == "retained")
        registry.prefetch(tiled, title: "Tiled", location: location, completed: false)
        #expect(registry.recentTasks == [tiled])
        #expect(!tile.requiresLocationValidation)
        registry.open(WorkWorkspaceKey(repository: "repo", kind: .task, id: "visited"), title: "Visited", location: location)
        #expect(tile.hostUse == "retained")
    }

    @Test("Repository and Wave primaries stay warm beside the Task bound; Close releases until a deliberate visit")
    func primariesStayWarmBesideTheTaskBound() {
        let registry = SessionsWorkspaceRegistry(localMachineId: "local")
        let location = WorkspaceIdentity(machineId: "local", worktree: "/tmp/repo")
        let repository = WorkWorkspaceKey(repository: "repo", kind: .repository, id: "repo")
        let wave = WorkWorkspaceKey(repository: "repo", kind: .wave, id: "wave-1")
        registry.prefetchPrimary(repository, title: "repo", location: location)
        registry.prefetchPrimary(wave, title: "product", wave: "product", location: location)
        registry.prefetchPrimary(wave, title: "product", wave: "product", location: location)
        #expect(registry.warmPrimaries == [repository, wave])
        #expect(registry.retained(wave)?.wave == "product")
        let limit = SessionsWorkspaceRegistry.warmConversationLimit
        let keys = (0..<limit + 1).map { WorkWorkspaceKey(repository: "repo", kind: .task, id: "task-\($0)") }
        for key in keys { registry.prefetch(key, title: key.id, location: location, completed: false) }
        #expect(registry.recentTasks.count == limit)
        #expect(registry.warmWork == [repository, wave] + keys.prefix(limit))
        // Primaries report no checkout presence: `hostUse` belongs to Task checkouts.
        let primary = registry.retained(wave)!
        #expect(primary.conversationIntent == .start)
        primary.multiplexer.load(sessionId: "wave-conversation")
        primary.closePane(primary.multiplexer.sessionPane!.id)
        #expect(primary.conversationClosed)
        #expect(registry.warmPrimaries == [repository])
        registry.prefetchPrimary(wave, title: "product", wave: "product", location: location)
        #expect(registry.warmPrimaries == [repository], "refresh never undoes Close")
        registry.open(wave, title: "product", location: location)
        #expect(!primary.conversationClosed)
        #expect(registry.warmPrimaries == [repository, wave])
    }

    @Test("Saved unfinished Tasks fill spare warmth without evicting working Tasks", arguments: [false, true])
    func restoredTasksFillSpareWarmth(full: Bool) {
        let workingCount = SessionsWorkspaceRegistry.warmConversationLimit - (full ? 0 : 1)
        let repository = "repo_" + UUID().uuidString
        defer { UserDefaults.standard.removeObject(forKey: "warm-conversations-local-\(repository.lowercased())") }
        let saved = WorkWorkspaceKey(repository: repository, kind: .task, id: "saved")
        let location = WorkspaceIdentity(machineId: "local", worktree: "/tmp/task")
        let previous = SessionsWorkspaceRegistry(localMachineId: "local")
        previous.loadWarmIntent(repository: repository)
        previous.open(saved, title: "Saved", location: location)
        previous.prepareForQuit()

        let registry = SessionsWorkspaceRegistry(localMachineId: "local")
        registry.loadWarmIntent(repository: repository)
        let working = (0..<workingCount).map { WorkWorkspaceKey(repository: repository, kind: .task, id: "working-\($0)") }
        for key in working {
            registry.prefetch(key, title: key.id, location: location, completed: false)
        }
        registry.prefetch(saved, title: "Saved", location: location, completed: false)
        #expect(registry.recentTasks == working + (full ? [] : [saved]))
        #expect(registry.restoreCandidates.isEmpty)
        #expect(registry.retained(saved)?.requiresLocationValidation == (full ? nil : true))
    }

    @Test("Quit acknowledges settled presentation as away and preserves an existing deadline")
    func quitReleasesWithoutClose() async throws {
        let application = RepositoryWorkspaces(localMachineId: "local")
        let registry = application.registry(for: "repo")
        let key = WorkWorkspaceKey(repository: "repo", kind: .task, id: UUID().uuidString)
        let workspace = registry.open(key, title: "Task", location: WorkspaceIdentity(machineId: "local", worktree: "/checkout"))
        workspace.multiplexer.load(sessionId: "history")
        workspace.startPersistence(query: RegistryQuery { args, _ in
            guard args.contains("presence") else { return "null" }
            func value(_ flag: String) -> String? {
                args.firstIndex(of: flag).flatMap { args.indices.contains($0 + 1) ? args[$0 + 1] : nil }
            }
            let ownerJSON = try #require(value("--owner"))
            let owner = try JSONDecoder().decode(TaskHostOwner.self, from: Data(ownerJSON.utf8))
            #expect(owner.machineID == "local")
            #expect(owner.pid == UInt32(ProcessInfo.processInfo.processIdentifier))
            #expect(owner.startedAt == Int64(try #require(Perf.processStartedAt)))
            #expect(owner.settlesOnExit)
            let deadline = value("--state") == "closed" ? 0 : 900
            let reason = value("--native-settled") == "true" ? "null" : "\"Native resources remain\""
            return """
                {"task_id":"task","completion":"completed","checkout":"/checkout","branch":"task",
                 "due_at":\(deadline),"phase":"waiting","reason":\(reason)}
                """
        }, repoPath: "/repo")
        application.prepareForQuit()
        // AppKit may close windows during termination. That is still departure.
        registry.closePresentation()
        workspace.retain(true)
        await application.releaseForQuit()
        #expect(workspace.hostUse == "away")
        #expect(workspace.nativeSettled)
        #expect(workspace.retirement?.dueAt == 900)
        #expect(workspace.retirement?.reason == nil)
        #expect(workspace.retentionError == nil)
        #expect(workspace.multiplexer.sessionPane?.sessionId == "history")
    }

    @Test("Failed or timed-out quit reporting never invents acknowledgement", arguments: [false, true])
    func quitReleaseFailure(stalled: Bool) async {
        let application = RepositoryWorkspaces(localMachineId: "local")
        let registry = application.registry(for: "repo")
        let key = WorkWorkspaceKey(repository: "repo", kind: .task, id: UUID().uuidString)
        let workspace = registry.open(key, title: "Task", location: WorkspaceIdentity(machineId: "local", worktree: "/checkout"))
        workspace.startPersistence(query: RegistryQuery { args, _ in
            guard args.contains("presence") else { return "null" }
            if stalled { try await Task.sleep(for: .seconds(60)) }
            throw RegistryQueryError("Destination unavailable; release unknown")
        }, repoPath: "/repo")
        application.prepareForQuit()
        let started = ContinuousClock.now
        await application.releaseForQuit(timeout: .milliseconds(20))
        #expect(started.duration(to: .now) < .seconds(2))
        #expect(workspace.hostUse == "away")
        #expect(workspace.retirement == nil)
        #expect(workspace.retentionError != nil)
    }

    @Test("Quitting preserves a prior explicit Close")
    func quitKeepsExplicitClose() async {
        let application = RepositoryWorkspaces(localMachineId: "local")
        let registry = application.registry(for: "repo")
        let key = WorkWorkspaceKey(repository: "repo", kind: .task, id: "closed")
        let workspace = registry.open(key, title: "Task", location: nil)
        workspace.closeWorkspace()
        application.prepareForQuit()
        await application.releaseForQuit()
        #expect(workspace.hostUse == "closed")
        #expect(workspace.nativeSettled)
    }

    @Test("Known repository entry presents its scene before admitting a new link",
          arguments: ["success", "failure", "cancellation"], [false, true])
    func repositoryEntryDuringResolution(outcome: String, restored: Bool) async throws {
        let path = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().path
        let router = WorkLinkRouter()
        let query = RegistryQuery { _, _ in #"{"id":"repo","locators":["repo"]}"# }
        let scene: RepositoryWorkspace
        if restored {
            scene = try await router.resolveWorkspace(RepositoryWorkspace(id: "repo", path: path), query: query)
        } else {
            scene = try await router.openRepository(path: path, link: nil, query: query) { _ in }
        }
        let registry = SessionsWorkspaceRegistry(localMachineId: "local")
        let model = WorkModel(query: query, repoPath: scene.path)
        model.bindWorkspaces(registry, repository: scene.id)
        let started = AsyncStream<Void>.makeStream(), release = AsyncStream<Void>.makeStream()
        let delivered = AsyncStream<LinkedWork>.makeStream()
        defer {
            started.continuation.finish()
            release.continuation.finish()
            delivered.continuation.finish()
        }
        var opened: [RepositoryWorkspace] = []
        var received: [LinkedWork] = []
        let window = UUID()
        defer { router.remove(window, repository: scene.id) }
        let link = LinkedWork.task(TaskLink(issue: "NEW", repo: path))
        let opening = Task {
            try await router.openRepository(path: path, link: link, query: RegistryQuery { _, _ in
                started.continuation.yield(())
                for await _ in release.stream { break }
                if outcome == "failure" { throw RegistryQueryError("Inventory unavailable") }
                return #"{"id":"repo","locators":["repo"]}"#
            }) { reopened in
                opened.append(reopened)
                router.register(window, repository: reopened.id,
                    openingRequests: router.workspaceRequests(reopened.id), focus: {},
                    inspect: { repository, incarnation in
                        model.inspectDesktop(repository: repository, window: incarnation, workspaces: registry)
                    }, perform: { try DesktopAction.perform($0, model: model, workspaces: registry) },
                    readText: { try registry.readText($0) }) { url in
                        received.append(url)
                        delivered.continuation.yield(url)
                    }
            }
        }
        for await _ in started.stream { break }
        #expect(opened == [scene])
        #expect(router.inspect().windows.map(\.window) == [window.uuidString])
        #expect(router.inspect().openings.first?.status == .opening)
        #expect(received.isEmpty)

        if outcome == "cancellation" { opening.cancel() }
        release.continuation.finish()
        switch await opening.result {
        case .success(let reopened):
            #expect(outcome == "success")
            #expect(reopened == scene)
            for await _ in delivered.stream { break }
            #expect(received == [.task(TaskLink(issue: "NEW", repo: scene.path))])
        case .failure(let error):
            #expect(outcome != "success")
            #expect(router.inspect().openings.first?.status == .failed)
            if outcome == "cancellation" { #expect(error is CancellationError) }
            #expect(received.isEmpty)
        }
        #expect(opened == [scene])
        #expect(router.retainedWorkspace(scene.id) == scene)
    }

    @Test("Reopened content stays visible while resolution stalls or fails", arguments: [false, true])
    func reopenDuringResolution(identityChanged: Bool) async throws {
        let path = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().path
        let router = WorkLinkRouter()
        let scene = try await router.openRepository(path: path, link: nil,
            query: RegistryQuery { _, _ in #"{"id":"repo","locators":["repo"]}"# }) { _ in }
        let application = RepositoryWorkspaces(localMachineId: "local")
        let registry = application.registry(for: scene.id)
        // Work opened earlier stays retained after another destination replaces it.
        let hidden = WorkWorkspaceKey(repository: scene.id, kind: .task, id: "hidden")
        registry.open(hidden, title: "Hidden", location: nil)
            .multiplexer.load(sessionId: "hidden-session")
        let key = WorkWorkspaceKey(repository: scene.id, kind: .task, id: "task")
        let workspace = registry.open(key, title: "Task", location: nil)
        workspace.multiplexer.load(sessionId: "session")
        workspace.presentation.selectedSessionId = "session"
        let noLaunch = RegistryQuery { args, _ in
            Issue.record("Rendering retained content must not prepare or launch: \(args)")
            throw RegistryQueryError("Unexpected operation")
        }
        _ = registry.sessionStore(repository: scene.id, repoPath: scene.path, query: noLaunch)
        let draft = workspace.files(issue: key.id, cwd: scene.path, query: noLaunch)
            .document("notes.md")
        draft.editor.string = "Unsubmitted text"
        let before = registry.inspect()
        let layout = registry.openWork
        let started = AsyncStream<Void>.makeStream()
        let release = AsyncStream<Void>.makeStream()
        let resolution = Task {
            try await router.resolveWorkspace(scene, query: RegistryQuery { _, _ in
                started.continuation.yield(())
                for await _ in release.stream { break }
                if identityChanged { return #"{"id":"another","locators":["another"]}"# }
                throw RegistryQueryError("Inventory unavailable")
            })
        }
        for await _ in started.stream { break }

        // Inspect the real reopened shell while its identity read has not returned.
        let shell = RepositoryWorkspaceView(workspace: scene, portfolioService: PortfolioService(query: noLaunch),
            workspaces: application, query: noLaunch, router: router, openRepository: { _, _ in
                Issue.record("Retained content must not open another repository")
            })
        let repo = try shell.inspect().find(RepoView.self).actualView()
        let sessions = try repo.inspect().find(SessionsView.self).actualView()
        let content = try sessions.inspect().find(SessionsContentView.self).actualView()
        #expect(content.model.selection == key.reference)
        #expect(content.model.navigation.selectedSessionId == "session")
        #expect(sessions.workspaces === registry)
        #expect(registry.inspect() == before)
        #expect(registry.openWork == layout)
        #expect(draft.editor.string == "Unsubmitted text")

        release.continuation.finish()
        let reason: String
        switch await resolution.result {
        case .success:
            Issue.record("Resolution unexpectedly succeeded")
            return
        case .failure(let error): reason = error.localizedDescription
        }
        var retries = 0
        let failed = RepositoryWorkspaceContent(workspace: router.retainedWorkspace(scene.id),
            error: reason, retry: { retries += 1 }) { resolved in
                #expect(resolved.id == scene.id)
                #expect(resolved.path == scene.path)
                return repo
            }
        #expect(throws: Never.self) { try failed.inspect().find(RepoView.self) }
        let banner = try failed.body.inspect().vStack().hStack(1)
        #expect(try banner.text(0).string() == reason)
        try banner.button(1).tap()
        #expect(retries == 1)
        #expect(registry.inspect() == before)
        #expect(registry.openWork == layout)
        #expect(draft.editor.string == "Unsubmitted text")
        #expect(router.retainedWorkspace("another") == nil)
    }

    @Test("An unresolved restored locator cannot mount a retained registry")
    func unresolvedScene() throws {
        let application = RepositoryWorkspaces(localMachineId: "local")
        application.registry(for: "unknown").open(
            WorkWorkspaceKey(repository: "unknown", kind: .task, id: "task"), title: "Task", location: nil)
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Unavailable") }
        let shell = RepositoryWorkspaceView(workspace: RepositoryWorkspace(id: "unknown", path: "/repo"),
            portfolioService: PortfolioService(query: query), workspaces: application, query: query,
            router: WorkLinkRouter(), openRepository: { _, _ in })
        #expect(throws: Never.self) { try shell.inspect().find(ViewType.ProgressView.self) }
        #expect(throws: (any Error).self) { try shell.inspect().find(RepoView.self) }
    }

    @Test("Reopening a repository keeps twelve Work layouts and drafts before inventory arrives")
    func reopenRepository() throws {
        let application = RepositoryWorkspaces(localMachineId: "local")
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Inventory unavailable") }
        weak var closedWindow: WorkModel?
        weak var retainedRegistry: SessionsWorkspaceRegistry?
        var workspaces: [SessionsWorkspace] = []
        let before: [DesktopWorkspaceInspection]
        let arrangement: [WorkWorkspaceKey]
        let selected: WorkWorkspaceKey?
        do {
            let window = WorkModel(query: query, repoPath: "/repo")
            let registry = application.registry(for: "repo")
            closedWindow = window
            retainedRegistry = registry
            window.bindWorkspaces(registry, repository: "repo")
            for index in 0..<12 {
                let key = WorkWorkspaceKey(repository: "repo", kind: .task, id: "task-\(index)")
                let location = WorkspaceIdentity(machineId: index.isMultiple(of: 2) ? "local" : "remote",
                                                 worktree: "/checkout/\(index)")
                let workspace = registry.open(key, title: "Task \(index)", location: location)
                workspace.multiplexer.load(sessionId: "session-\(index)", location: location)
                let sessionPane = workspace.multiplexer.focusedPaneId
                let filesPane = try #require(workspace.multiplexer.showBrowser(.files, location: location))
                workspace.multiplexer.updateRatio(between: sessionPane, and: filesPane.id, ratio: 0.6)
                workspace.multiplexer.setCollapsed(paneId: filesPane.id, collapsed: true)
                workspace.presentation.selectedSessionId = "session-\(index)"
                workspace.presentation.content = .terminals
                let files = workspace.files(issue: key.id, cwd: location.worktree, query: query)
                files.document("notes.md").editor.string = "Unsubmitted draft \(index)"
                workspaces.append(workspace)
            }
            before = registry.inspect()
            arrangement = registry.openWork
            selected = registry.selectedWork
        }
        #expect(closedWindow == nil)
        #expect(retainedRegistry != nil)

        let reopened = application.registry(for: "repo")
        let nextWindow = WorkModel(query: query, repoPath: "/different-locator")
        nextWindow.bindWorkspaces(reopened, repository: "repo")
        #expect(reopened === retainedRegistry)
        #expect(reopened.openWork == arrangement)
        #expect(reopened.inspect() == before)
        #expect(nextWindow.selection == selected?.reference)
        #expect(nextWindow.navigation.selectedSessionId == "session-11")
        for (index, workspace) in workspaces.enumerated() {
            #expect(reopened.retained(workspace.key) === workspace)
            let files = workspace.files(issue: workspace.key.id,
                                        cwd: "/checkout/\(index)", query: query)
            #expect(files.document("notes.md").editor.string == "Unsubmitted draft \(index)")
            nextWindow.select(workspace.key.reference)
            #expect(reopened.retained(workspace.key)?.multiplexer === workspace.multiplexer)
            #expect(nextWindow.navigation.selectedSessionId == "session-\(index)")
        }
    }

    @Test("Closing every Work preserves hidden panes when a repository window reopens")
    func hiddenWorkStaysHidden() throws {
        let application = RepositoryWorkspaces(localMachineId: "local")
        let registry = application.registry(for: "repo")
        let key = WorkWorkspaceKey(repository: "repo", kind: .task, id: "task")
        let workspace = registry.open(key, title: "Task", location: nil)
        workspace.multiplexer.load(sessionId: "session")
        registry.clearPresentation()
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Inventory unavailable") }
        let window = WorkModel(query: query, repoPath: "/repo")
        window.bindWorkspaces(application.registry(for: "repo"), repository: "repo")
        #expect(registry.openWork.isEmpty)
        #expect(registry.selectedWork == nil)
        #expect(registry.retained(key) === workspace)
        #expect(workspace.multiplexer.focusedPane.sessionId == "session")
    }

    @Test("Repository windows retain separate surface pools and Work")
    func repositoriesAreIsolated() {
        let application = RepositoryWorkspaces()
        let first = application.registry(for: "first")
        let second = application.registry(for: "second")
        #expect(first !== second)
        #expect(first.surfaces !== second.surfaces)
        let key = WorkWorkspaceKey(repository: "first", kind: .task, id: "task")
        first.open(key, title: "Task", location: nil)
        #expect(second.keys.isEmpty)
        #expect(application.registry(for: "first") === first)
    }

    @Test("A reopened scene preserves the shared repository identity learned before closing")
    func associatedRepository() throws {
        let application = RepositoryWorkspaces()
        let registry = application.registry(for: "scene")
        let oldKey = WorkWorkspaceKey(repository: "scene", kind: .task, id: "task")
        let workspace = registry.open(oldKey, title: "Task", location: nil)
        workspace.multiplexer.load(sessionId: "session")
        registry.reassociateRepository(from: "scene", to: "shared")
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Inventory unavailable") }
        let window = WorkModel(query: query, repoPath: "/repo")
        window.bindWorkspaces(application.registry(for: "scene"), repository: "scene")
        window.select(.task(id: "task"))
        #expect(registry.keys == [WorkWorkspaceKey(repository: "shared", kind: .task, id: "task")])
        #expect(registry.retained(try #require(registry.selectedWork)) === workspace)
    }
}
#endif
