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
        let delivered = AsyncStream<URL>.makeStream()
        defer {
            started.continuation.finish()
            release.continuation.finish()
            delivered.continuation.finish()
        }
        var opened: [RepositoryWorkspace] = []
        var received: [URL] = []
        let window = UUID()
        defer { router.remove(window, repository: scene.id) }
        let link = try #require(TaskLink(issue: "NEW", repo: path).url)
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
                    }, controlPane: { try registry.controlPane($0, model: model) },
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
            #expect(received == [TaskLink(issue: "NEW", repo: scene.path).url])
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
        let key = WorkWorkspaceKey(repository: scene.id, kind: .task, id: "task")
        let workspace = registry.open(key, title: "Task", location: nil)
        workspace.multiplexer.load(sessionId: "session")
        workspace.presentation.selectedSessionId = "session"
        let hidden = WorkWorkspaceKey(repository: scene.id, kind: .task, id: "hidden")
        registry.open(hidden, title: "Hidden", location: nil, besideFocused: true)
            .multiplexer.load(sessionId: "hidden-session")
        registry.hide(hidden)
        let noLaunch = RegistryQuery { args, _ in
            Issue.record("Rendering retained content must not prepare or launch: \(args)")
            throw RegistryQueryError("Unexpected operation")
        }
        _ = registry.sessionStore(repository: scene.id, repoPath: scene.path, query: noLaunch)
        let draft = workspace.files(issue: key.id, cwd: scene.path, query: noLaunch)
            .document("notes.md")
        draft.editor.string = "Unsubmitted text"
        let before = registry.inspect()
        let layout = registry.layout
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
        #expect(registry.layout == layout)
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
        #expect(registry.layout == layout)
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
        let arrangement: WorkLayout?
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
                let workspace = registry.open(key, title: "Task \(index)", location: location,
                                              besideFocused: true)
                workspace.multiplexer.load(sessionId: "session-\(index)", location: location)
                let sessionPane = workspace.multiplexer.focusedPaneId
                let filesPane = try #require(workspace.multiplexer.show(
                    .files(taskId: key.id, location: location), beside: sessionPane))
                workspace.multiplexer.updateRatio(between: sessionPane, and: filesPane.id, ratio: 0.6)
                workspace.multiplexer.setCollapsed(paneId: filesPane.id, collapsed: true)
                workspace.presentation.selectedSessionId = "session-\(index)"
                workspace.presentation.content = .terminals
                let files = workspace.files(issue: key.id, cwd: location.worktree, query: query)
                files.document("notes.md").editor.string = "Unsubmitted draft \(index)"
                workspaces.append(workspace)
            }
            before = registry.inspect()
            arrangement = registry.layout
            selected = registry.selectedWork
        }
        #expect(closedWindow == nil)
        #expect(retainedRegistry != nil)

        let reopened = application.registry(for: "repo")
        let nextWindow = WorkModel(query: query, repoPath: "/different-locator")
        nextWindow.bindWorkspaces(reopened, repository: "repo")
        #expect(reopened === retainedRegistry)
        #expect(reopened.layout == arrangement)
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
        registry.hide(key)
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
