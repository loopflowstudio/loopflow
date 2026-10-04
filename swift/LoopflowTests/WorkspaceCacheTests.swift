#if os(macOS)
import Foundation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Workspace cache")
@MainActor
struct WorkspaceCacheTests {
    private static let repo = "/src/loopflow"

    @Test("A returning launch shows the saved workspace and selection before any read finishes")
    func returningLaunchRestores() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let cache = WorkspaceCache(directory: directory)
        let first = PodiumModel(query: source.query, repoPath: Self.repo, cache: cache)
        #expect(first.workspaceStatus == .loading)
        await first.refresh()
        #expect(first.workspaceStatus == .current)
        first.select(.task(id: "issue-now"))
        first.confirmHome("home-a")
        cache.flush()

        let offline = RegistryQuery { _, _ in throw RegistryQueryError("offline") }
        let returning = PodiumModel(query: offline, repoPath: Self.repo, cache: WorkspaceCache(directory: directory))

        #expect(returning.workspaceStatus == .updating)
        #expect(returning.savedHomeId == "home-a")
        #expect(returning.roadmap.value?.waves.map(\.wave.id) == first.roadmap.value?.waves.map(\.wave.id))
        #expect(returning.sessions.value?.map(\.id) == first.sessions.value?.map(\.id))
        #expect(returning.selection == .task(id: "issue-now"))
        #expect(returning.task(id: "issue-now") != nil)
    }

    @Test("Saved text never claims a live process or a legal mutation")
    func savedTextIsQuiet() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let saving = WorkspaceCache(directory: directory)
        let first = PodiumModel(query: source.query, repoPath: Self.repo, cache: saving)
        await first.refresh()
        let liveFlow = try #require(first.task(id: "issue-now")?.task.flow)
        guard case .pinned(let live) = liveFlow.record else { Issue.record("fixture Flow is not pinned"); return }
        #expect(live.execution == .running)
        #expect(first.sessions.value?.first?.state == .active)
        #expect(first.task(id: "issue-now")?.task.condition.reason != WorkspaceCache.savedReason)
        saving.flush()

        let returning = PodiumModel(query: RegistryQuery { _, _ in throw RegistryQueryError("offline") },
                                    repoPath: Self.repo, cache: WorkspaceCache(directory: directory))
        let flow = try #require(returning.task(id: "issue-now")?.task.flow)
        guard case .pinned(let pinned) = flow.record else { Issue.record("saved Flow is not pinned"); return }
        #expect(pinned.execution == .unknown)
        #expect(pinned.current == live.current)
        #expect(flow.controls.allSatisfy { $0.unavailable == WorkspaceCache.savedReason })
        let conditions = returning.roadmap.value?.waves.flatMap { $0.tasks.items.map(\.condition) } ?? []
        #expect(!conditions.isEmpty)
        #expect(conditions.allSatisfy { $0.state == .unknown && $0.reason == WorkspaceCache.savedReason })
        let session = try #require(returning.sessions.value?.first)
        #expect(session.state == .unknown)
        #expect(session.actions.map(\.kind) == [.open])
    }

    @Test("A failed refresh keeps the saved workspace under one truthful status")
    func failedRefreshKeepsSaved() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let saving = WorkspaceCache(directory: directory)
        await PodiumModel(query: source.query, repoPath: Self.repo, cache: saving).refresh()
        saving.flush()

        await source.fail()
        let returning = PodiumModel(query: source.query, repoPath: Self.repo, cache: WorkspaceCache(directory: directory))
        await returning.refresh()

        #expect(returning.workspaceStatus == .failed("Couldn't update: offline"))
        #expect(returning.roadmap.value != nil)
        #expect(returning.sessions.value?.isEmpty == false)
        let navigator = WorkspaceNavigator(model: returning, onOpenSession: { _ in })
        let status = try navigator.inspect().findAll(where: { (try? $0.accessibilityIdentifier()) == "workspace-status" })
        #expect(status.count == 1)

        await source.recover()
        await returning.refresh()
        #expect(returning.workspaceStatus == .current)
        guard case .pinned(let pinned) = returning.task(id: "issue-now")?.task.flow.record else {
            Issue.record("fresh Flow is not pinned"); return
        }
        #expect(pinned.execution == .running)
    }

    @Test("A first launch has one loading message")
    func firstLaunchLoadsOnce() throws {
        let model = PodiumModel(query: RegistryQuery { _, _ in throw RegistryQueryError("unused") },
                                repoPath: Self.repo, cache: WorkspaceCache(directory: try temporaryDirectory()))
        let navigator = WorkspaceNavigator(model: model, onOpenSession: { _ in })
        let texts = try navigator.inspect().findAll(ViewType.Text.self).map { try $0.string() }
        #expect(texts.filter { $0.contains("…") } == ["Loading workspace…"])
    }

    @Test("Absent, corrupt and incompatible files load as nothing and are removed")
    func unusableFilesRecover() throws {
        let directory = try temporaryDirectory()
        let file = directory.appendingPathComponent("workspace.json")
        #expect(WorkspaceCache(directory: directory).load() == nil)

        for text in ["{ not json", #"{"version":0,"snapshot":{"repositories":{}}}"#] {
            try Data(text.utf8).write(to: file)
            #expect(WorkspaceCache(directory: directory).load() == nil)
            #expect(!FileManager.default.fileExists(atPath: file.path))
        }

        try Data(#"{"version":1,"snapshot":{"roadmap":"{}","repositories":{}}}"#.utf8).write(to: file)
        let model = PodiumModel(query: RegistryQuery { _, _ in throw RegistryQueryError("unused") },
                                repoPath: Self.repo, cache: WorkspaceCache(directory: directory))
        #expect(model.workspaceStatus == .loading)
    }

    @Test("A workspace saved under another Home is dropped")
    func anotherHomeInvalidates() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let saving = WorkspaceCache(directory: directory)
        let first = PodiumModel(query: source.query, repoPath: Self.repo, cache: saving)
        await first.refresh()
        first.select(.task(id: "issue-now"))
        first.confirmHome("home-a")
        saving.flush()

        let cache = WorkspaceCache(directory: directory)
        let returning = PodiumModel(query: RegistryQuery { _, _ in throw RegistryQueryError("offline") },
                                    repoPath: Self.repo, cache: cache)
        returning.confirmHome("home-b")
        cache.flush()

        #expect(returning.workspaceStatus == .loading)
        #expect(returning.sessions.value == nil)
        #expect(returning.selection == nil)
        #expect(returning.task(id: "issue-now") == nil)
        let saved = try #require(WorkspaceCache(directory: directory).load())
        #expect(saved == WorkspaceSnapshot(homeId: "home-b"))
    }

    @Test("A Session read that finishes after a repository switch saves nothing")
    func staleSessionReadIsNotSaved() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let cache = WorkspaceCache(directory: directory)
        let model = PodiumModel(query: source.query, repoPath: Self.repo, cache: cache)
        await source.hold("session")
        let read = Task { await model.refreshSessions() }
        await source.waitForHeldRead()
        model.setRepoPath("/src/other")
        await source.release()
        await read.value
        cache.flush()

        #expect(model.sessions.value == nil)
        #expect(WorkspaceCache(directory: directory).load()?.repositories[Self.repo]?.sessionPages == nil)
    }

    @Test("A slow provider keeps the saved workspace usable under Updating…")
    func slowProviderKeepsSaved() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let saving = WorkspaceCache(directory: directory)
        await PodiumModel(query: source.query, repoPath: Self.repo, cache: saving).refresh()
        saving.flush()

        let returning = PodiumModel(query: source.query, repoPath: Self.repo, cache: WorkspaceCache(directory: directory))
        await source.hold("roadmap")
        let read = Task { await returning.refresh() }
        await source.waitForHeldRead()

        #expect(returning.workspaceStatus == .updating)
        #expect(returning.task(id: "issue-now") != nil)
        #expect(returning.sessions.value?.isEmpty == false)
        let navigator = WorkspaceNavigator(model: returning, onOpenSession: { _ in })
        let texts = try navigator.inspect().findAll(ViewType.Text.self).map { try $0.string() }
        #expect(texts.filter { $0.contains("…") } == ["Updating…"])

        await source.release()
        await read.value
        #expect(returning.workspaceStatus == .current)
    }

    @Test("A refresh that finishes after a selection change keeps the newer selection")
    func refreshAfterSelectionChange() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let saving = WorkspaceCache(directory: directory)
        let first = PodiumModel(query: source.query, repoPath: Self.repo, cache: saving)
        await first.refresh()
        first.select(.task(id: "issue-now"))
        saving.flush()

        let cache = WorkspaceCache(directory: directory)
        let returning = PodiumModel(query: source.query, repoPath: Self.repo, cache: cache)
        #expect(returning.selection == .task(id: "issue-now"))
        await source.hold("roadmap")
        let read = Task { await returning.refresh() }
        await source.waitForHeldRead()
        returning.select(.wave(id: "wave-1"))
        await source.release()
        await read.value
        cache.flush()

        #expect(returning.selection == .wave(id: "wave-1"))
        #expect(returning.navigation.content == .details)
        #expect(returning.workspaceStatus == .current)
        #expect(WorkspaceCache(directory: directory).load()?.repositories[Self.repo]?.selection == .wave(id: "wave-1"))
    }

    @Test("Another window in the same process opens from the workspace already held")
    func warmReopen() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let cache = WorkspaceCache(directory: directory)
        let first = PodiumModel(query: source.query, repoPath: Self.repo, cache: cache)
        await first.refresh()
        cache.flush()
        try FileManager.default.removeItem(at: directory.appendingPathComponent("workspace.json"))

        let reads = await source.reads
        let reopened = PodiumModel(query: source.query, repoPath: Self.repo, cache: cache)

        #expect(reopened.workspaceStatus == .updating)
        #expect(reopened.roadmap.value?.waves.map(\.wave.id) == first.roadmap.value?.waves.map(\.wave.id))
        #expect(reopened.sessions.value?.map(\.id) == first.sessions.value?.map(\.id))
        #expect(await source.reads == reads)
    }

    @Test("A saved launch repository opens without git and is checked afterwards")
    func savedLaunchRepository() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let saving = WorkspaceCache(directory: directory)
        await PodiumModel(query: source.query, repoPath: Self.repo, cache: saving).refresh()
        saving.flush()

        // `/src/loopflow` is no repository here, so only the saved workspace can scope the window.
        let unknown = PodiumModel(query: source.query, launchCandidates: [Self.repo],
                                  cache: WorkspaceCache(directory: try temporaryDirectory()))
        #expect(unknown.repoPath == nil)
        let returning = PodiumModel(query: source.query, launchCandidates: ["/src/missing", Self.repo],
                                    cache: WorkspaceCache(directory: directory))
        #expect(returning.repoPath == Self.repo)
        #expect(returning.sessions.value?.isEmpty == false)

        await returning.refreshPortfolio(initialRepoPath: nil)
        #expect(returning.repoPath == nil)
    }

    @Test("Explicit history is read on request, never saved")
    func headlessListingIsNotSaved() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let cache = WorkspaceCache(directory: directory)
        let model = PodiumModel(query: source.query, repoPath: Self.repo, cache: cache)
        model.navigation.showsHeadlessSessions = true
        await model.refreshSessions()
        cache.flush()

        #expect(model.sessions.value?.isEmpty == false)
        #expect(WorkspaceCache(directory: directory).load()?.repositories[Self.repo]?.sessionPages == nil)
    }

    @Test("Only the most recently saved repositories are kept")
    func repositoriesAreBounded() throws {
        let directory = try temporaryDirectory()
        let cache = WorkspaceCache(directory: directory)
        let start = Date(timeIntervalSince1970: 1_000)
        for index in 0..<(WorkspaceCache.maxRepositories + 3) {
            cache.saveSelection(.wave(id: "wave"), repo: "/src/\(index)", at: start.addingTimeInterval(Double(index)))
        }
        cache.flush()

        let saved = try #require(WorkspaceCache(directory: directory).load())
        #expect(saved.repositories.count == WorkspaceCache.maxRepositories)
        #expect(saved.repositories["/src/0"] == nil)
        #expect(saved.repositories["/src/\(WorkspaceCache.maxRepositories + 2)"] != nil)
    }

    @Test("An unchanged poll writes nothing")
    func unchangedPollDoesNotRewrite() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let cache = WorkspaceCache(directory: directory)
        let model = PodiumModel(query: source.query, repoPath: Self.repo, cache: cache)
        await model.refreshSessions()
        cache.flush()
        let file = directory.appendingPathComponent("workspace.json")
        let written = try FileManager.default.attributesOfItem(atPath: file.path)[.modificationDate] as? Date
        try await Task.sleep(for: .milliseconds(20))
        await model.refreshSessions()
        cache.flush()

        #expect(try FileManager.default.attributesOfItem(atPath: file.path)[.modificationDate] as? Date == written)
    }

    private func temporaryDirectory() throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("workspace-cache-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }
}

private actor Source {
    private let roadmap: String
    private let sessions: String
    private var failed = false
    private var holding: String?
    private(set) var reads = 0
    private var held: CheckedContinuation<Void, Never>?
    private var arrived: CheckedContinuation<Void, Never>?

    init() throws {
        let fixtures = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto")
        roadmap = try String(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json"), encoding: .utf8)
        sessions = try String(contentsOf: fixtures.appendingPathComponent("session_page.json"), encoding: .utf8)
    }

    nonisolated var query: RegistryQuery { RegistryQuery { args, _ in try await self.read(args) } }

    func fail() { failed = true }
    func recover() { failed = false }
    func hold(_ verb: String) { holding = verb }

    func waitForHeldRead() async {
        if held != nil { return }
        await withCheckedContinuation { arrived = $0 }
    }

    func release() {
        holding = nil
        held?.resume()
        held = nil
    }

    private func read(_ args: [String]) async throws -> String {
        reads += 1
        if failed { throw RegistryQueryError("offline") }
        if holding == args.first {
            await withCheckedContinuation { continuation in
                held = continuation
                arrived?.resume()
                arrived = nil
            }
        }
        switch args.first {
        case "roadmap": return roadmap
        case "session": return sessions
        case "wave": return "[]"
        case "activity": return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
        default: throw RegistryQueryError("unexpected command")
        }
    }
}
#endif
