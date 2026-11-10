#if os(macOS)
import Foundation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Work cache")
@MainActor
struct WorkCacheTests {
    private static let repo = "/src/loopflow"

    @Test("A delayed same-path Session read cannot populate a replacement Machine")
    func sessionReadKeepsMachineIdentity() async throws {
        let source = try Source()
        let model = WorkModel(query: source.query, repoPath: Self.repo)
        model.confirmMachine("machine-a")
        await model.refreshSessions()
        #expect(model.sessions.value?.isEmpty == false)
        await source.hold(["session", "list"])
        let pending = Task { await model.refreshSessions() }
        await source.waitForHeldRead()
        model.confirmMachine("machine-b")
        #expect(model.sessions.value == nil)
        await source.release()
        await pending.value
        #expect(model.sessions.value == nil)
        await model.refreshSessions()
        #expect(model.sessions.value?.isEmpty == false)
    }

    @Test("A returning launch restores its cache before any read finishes")
    func returningLaunchRestores() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let cache = WorkCache(directory: directory)
        let first = WorkModel(query: source.query, repoPath: Self.repo, cache: cache)
        first.confirmMachine("home-a")
        #expect(first.workStatus == .loading)
        await first.refresh()
        #expect(first.workStatus == .current)
        first.select(.task(id: "issue-review"))
        cache.flush()

        let cacheURL = directory.appendingPathComponent("workspace.json")
        let savedBytes = try Data(contentsOf: cacheURL)

        let offline = RegistryQuery { _, _ in throw RegistryQueryError("offline") }
        let returning = WorkModel(query: offline, repoPath: Self.repo, cache: WorkCache(directory: directory))

        #expect(returning.workStatus == .updating)
        #expect(returning.savedMachineId == "home-a")
        #expect(returning.roadmap.value?.waves.map(\.wave.id) == first.roadmap.value?.waves.map(\.wave.id))
        #expect(returning.sessions.value?.map(\.id) == first.sessions.value?.map(\.id))
        #expect(returning.selection == .task(id: "issue-review"))
        #expect(returning.task(id: "issue-now") != nil)
        #expect(try Data(contentsOf: cacheURL) == savedBytes)
    }

    @Test("Restart keeps the selected Task but does not retain stale completion or location", arguments: [false, true])
    func restartWaitsForFreshEvidence(machineUnavailable: Bool) async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let cache = WorkCache(directory: directory)
        let first = WorkModel(query: source.query, repoPath: Self.repo, cache: cache)
        first.confirmMachine("home-a")
        await first.refresh()
        first.select(.task(id: "issue-review"))
        cache.flush()
        let repository = "repo_" + UUID().uuidString
        defer { UserDefaults.standard.removeObject(forKey: "warm-conversations-home-a-\(repository.lowercased())") }
        let selectedTask = try #require(first.task(id: "issue-review")?.task)
        let otherTask = try #require(first.task(id: "issue-now")?.task)
        let selected = WorkWorkspaceKey(repository: repository, kind: .task, id: try #require(selectedTask.runtime?.workId))
        let other = WorkWorkspaceKey(repository: repository, kind: .task, id: try #require(otherTask.runtime?.workId))
        let previous = SessionsWorkspaceRegistry(localMachineId: "home-a")
        previous.loadWarmIntent(repository: repository)
        previous.open(other, title: "Other", location: otherTask.reference.workspace?.identity)
        previous.open(selected, title: "Selected", location: selectedTask.reference.workspace?.identity)
        previous.prepareForQuit()

        try await source.completeTask("issue-now")
        await source.setLocationFailure(machineUnavailable ? "Machine unavailable" : "Recorded checkout changed")
        let returning = WorkModel(query: source.query, repoPath: Self.repo, cache: WorkCache(directory: directory))
        returning.confirmMachine("home-a")
        let registry = SessionsWorkspaceRegistry(localMachineId: "home-a")
        returning.bindWorkspaces(registry, repository: repository)
        #expect(returning.selection == .task(id: "issue-review"))
        #expect(registry.selectedWork == selected)
        #expect(registry.retained(other) == nil)
        #expect(registry.retained(selected)?.location == nil)
        #expect(registry.retained(selected)?.presented == false)
        await source.hold(["plan", "list"])
        let refresh = Task { await returning.refresh() }
        await source.waitForHeldRead()
        returning.openWarmConversations()
        #expect(registry.retained(other) == nil)
        #expect(registry.preparationCount == 0)
        await source.release()
        await refresh.value
        await spin { registry.retained(selected)?.conversationError == nil }
        #expect(returning.selection == .task(id: "issue-review"))
        #expect(registry.retained(selected)?.presented == false)
        #expect(registry.retained(selected)?.conversationError?.contains(machineUnavailable ? "Machine unavailable" : "location changed") == true)
        #expect(registry.retained(other) == nil)
        #expect(!registry.recentTasks.contains(other))
        registry.prepareForQuit()
    }

    @Test("Fresh restart prepares the saved conversations and honors saved Close", arguments: [false, true])
    func restartRevealsSelectedFirst(closed: Bool) async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let cache = WorkCache(directory: directory)
        let first = WorkModel(query: source.query, repoPath: Self.repo, cache: cache)
        first.confirmMachine("home-a")
        await first.refresh()
        first.select(.task(id: "issue-review"))
        cache.flush()
        let repository = "repo_" + UUID().uuidString
        defer { UserDefaults.standard.removeObject(forKey: "warm-conversations-home-a-\(repository.lowercased())") }
        let selectedTask = try #require(first.task(id: "issue-review")?.task)
        let otherTask = try #require(first.task(id: "issue-now")?.task)
        let selected = WorkWorkspaceKey(repository: repository, kind: .task, id: try #require(selectedTask.runtime?.workId))
        let other = WorkWorkspaceKey(repository: repository, kind: .task, id: try #require(otherTask.runtime?.workId))
        let previous = SessionsWorkspaceRegistry(localMachineId: "home-a")
        previous.loadWarmIntent(repository: repository)
        previous.open(other, title: "Other", location: otherTask.reference.workspace?.identity)
        let selectedWorkspace = previous.open(selected, title: "Selected", location: selectedTask.reference.workspace?.identity)
        if closed { selectedWorkspace.closePane(selectedWorkspace.multiplexer.focusedPaneId) }
        previous.prepareForQuit()
        await source.setLocationFailure(nil)
        let returning = WorkModel(query: source.query, repoPath: Self.repo, cache: WorkCache(directory: directory))
        returning.confirmMachine("home-a")
        let registry = SessionsWorkspaceRegistry(localMachineId: "home-a")
        returning.bindWorkspaces(registry, repository: repository)
        await returning.refresh()
        let store = registry.sessionStore(repository: repository, repoPath: Self.repo, query: source.query)
        let expected = closed ? 1 : 2
        await spin { store.preparedTaskConversations.count < expected }
        #expect(registry.selectedWork == selected)
        #expect(store.preparedTaskConversations.count == expected)
        // boundedSelectedFirstPreparation proves admission order; these two reads then race.
        #expect(Set(await source.preparedTasks) == (closed ? [other.id] : [selected.id, other.id]))
        #expect(registry.retained(selected)?.conversationClosed == closed)
        #expect(registry.retained(selected)?.presented == !closed)
        #expect(registry.retained(other)?.presented == true)
        // Saved unstarted and shipping Tasks are not working: they reveal history only.
        #expect(store.preparedTaskConversations.allSatisfy { $0.launchArguments.contains("--reveal-existing") })
        registry.prepareForQuit()
    }

    @Test("Fresh repository load prefetches working Tasks without saved visits and honors Close", arguments: [false, true])
    func loadPrefetchesWorkingTasks(closed: Bool) async throws {
        let source = try Source()
        try await source.populateWorkingTasks(owned: Prefetch.owned)
        await source.setLocationFailure(nil)
        await source.holdPreparations()
        let repository = "repo_" + UUID().uuidString
        defer { UserDefaults.standard.removeObject(forKey: "warm-conversations-home-a-\(repository.lowercased())") }
        let closedKey = WorkWorkspaceKey(repository: repository, kind: .task, id: Prefetch.newestOwned)
        if closed {
            let previous = SessionsWorkspaceRegistry(localMachineId: "home-a")
            previous.loadWarmIntent(repository: repository)
            let workspace = previous.open(closedKey, title: "Closed", location: nil)
            workspace.closePane(workspace.multiplexer.focusedPaneId)
            previous.prepareForQuit()
        }
        let registry = SessionsWorkspaceRegistry(localMachineId: "home-a")
        let model = WorkModel(query: source.query, repoPath: Self.repo)
        model.confirmMachine("home-a")
        model.bindWorkspaces(registry, repository: repository)
        #expect(registry.restoreCandidates.isEmpty)
        #expect(registry.recentTasks.isEmpty)
        await model.refresh()
        await spin { await source.preparedTasks.count < 2 }
        #expect(model.selection == nil)
        #expect(model.workStatus == .current) // Rendering is not held by preparation.
        #expect(registry.preparationCount == 2)
        // The newest row is the started Task nobody owns; it opens like any working Task.
        let limit = SessionsWorkspaceRegistry.warmConversationLimit
        let newest = Prefetch.owned - (closed ? 2 : 1)
        let expected = [Prefetch.unowned] + stride(from: newest, through: newest - (limit - 2), by: -1).map { "prefetch-\($0)" }
        #expect(registry.recentTasks.map(\.id) == expected)
        #expect(Set(await source.preparedTasks) == Set(expected.prefix(2)))
        await source.releasePreparations()
        let store = registry.sessionStore(repository: repository, repoPath: Self.repo, query: source.query)
        await spin { store.preparedTaskConversations.count < limit }
        #expect(store.preparedTaskConversations.count == limit)
        #expect(Set(await source.preparedTasks) == Set(expected))
        #expect(store.sessions.filter { $0.state == .prepared }.allSatisfy { $0.launchArguments.contains("--start-if-free") })
        model.openWarmConversations()
        #expect(registry.recentTasks.count == SessionsWorkspaceRegistry.warmConversationLimit)
        if closed { #expect(registry.retained(closedKey) == nil) }
        registry.prepareForQuit()
    }

    @Test("A failed load Start shows its failure, never a Start offer")
    func failedPrefetchKeepsItsReason() async throws {
        let source = try Source()
        await source.setPreparationFailure("Claude account unavailable")
        let repository = "repo_" + UUID().uuidString
        defer { UserDefaults.standard.removeObject(forKey: "warm-conversations-home-a-\(repository.lowercased())") }
        let (registry, model) = try await loadWorkingTasks(from: source, repository: repository)
        let key = WorkWorkspaceKey(repository: repository, kind: .task, id: Prefetch.newestOwned)
        await spin { registry.retained(key)?.conversationError == nil }
        let workspace = try #require(registry.retained(key))
        #expect(workspace.conversationError == "Claude account unavailable")
        #expect(workspace.conversationStart == nil)
        registry.prepareForQuit()
    }

    @Test("A working Task behind both preparation slots shows opening, never Open conversation")
    func queuedWorkingTaskShowsOpening() async throws {
        let source = try Source()
        await source.holdPreparations()
        let repository = "repo_" + UUID().uuidString
        defer { UserDefaults.standard.removeObject(forKey: "warm-conversations-home-a-\(repository.lowercased())") }
        let (registry, model) = try await loadWorkingTasks(from: source, repository: repository)
        await spin { await source.preparedTasks.count < 2 }
        #expect(registry.preparationCount == 2)
        let prepared = await source.preparedTasks
        let queued = try #require(registry.recentTasks.first { !prepared.contains($0.id) })
        let workspace = try #require(registry.retained(queued))
        #expect(!workspace.openingConversation)
        let store = registry.sessionStore(repository: repository, repoPath: Self.repo, query: source.query)
        let view = WorkspacePanesView(workspace: workspace, isFocused: true, sessions: store, model: model,
            onFocus: { id, _ in workspace.multiplexer.setFocusedPane(id) })
        #expect(throws: (any Error).self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "open-conversation")
        }
        _ = try view.inspect().find(text: "Opening… Input paused.")

        await source.releasePreparations()
        await spin { await source.releasePreparations(); return await !source.preparedTasks.contains(queued.id) }
        #expect(await source.preparedTasks.contains(queued.id))
        registry.prepareForQuit()
    }

    @Test("Close and a failure keep a person's next action")
    func closedOrFailedTaskStopsAwaitingOpening() async throws {
        let source = try Source()
        await source.holdPreparations()
        let repository = "repo_" + UUID().uuidString
        defer { UserDefaults.standard.removeObject(forKey: "warm-conversations-home-a-\(repository.lowercased())") }
        let (registry, model) = try await loadWorkingTasks(from: source, repository: repository)
        await spin { await source.preparedTasks.count < 2 }
        let prepared = await source.preparedTasks
        let queued = try #require(registry.recentTasks.first { !prepared.contains($0.id) })
        let workspace = try #require(registry.retained(queued))
        #expect(model.conversationIsOpening(workspace))
        workspace.conversationError = "Machine unavailable"
        #expect(!model.conversationIsOpening(workspace))
        workspace.conversationError = nil
        workspace.conversationClosed = true
        #expect(!model.conversationIsOpening(workspace))
        await source.releasePreparations()
        registry.prepareForQuit()
    }

    @Test("A started Task replaces its refused reveal with an automatic Start")
    func startedTaskLeavesRefusedReveal() async throws {
        let source = try Source()
        let repository = "repo_" + UUID().uuidString
        defer { UserDefaults.standard.removeObject(forKey: "warm-conversations-home-a-\(repository.lowercased())") }
        let (registry, model) = try await loadWorkingTasks(from: source, repository: repository)
        await spin { await source.preparedTasks.count < SessionsWorkspaceRegistry.warmConversationLimit }
        await spin { registry.preparationCount > 0 }
        await source.setPreparationFailure("No provider history")
        let id = Prefetch.unstarted
        let workspace = registry.open(WorkWorkspaceKey(repository: repository, kind: .task, id: id), title: "Refused", location: nil)
        model.openWarmConversations()
        await spin { workspace.conversationError == nil }
        #expect(workspace.conversationError == "No provider history")
        #expect(workspace.conversationStart != nil)
        #expect(!model.conversationIsOpening(workspace))

        await source.setPreparationFailure(nil)
        try await source.startTask(id)
        await model.refresh()
        await spin { await !source.preparedTasks.contains(id) }
        await spin { workspace.openingConversation }
        #expect(await source.preparedTasks.contains(id))
        #expect(workspace.conversationError == nil)
        #expect(workspace.conversationStart == nil)
        registry.prepareForQuit()
    }

    @Test("A reveal refused after its Task started leaves no offer and starts")
    func supersededRefusalDoesNotLand() async throws {
        let source = try Source()
        let repository = "repo_" + UUID().uuidString
        defer { UserDefaults.standard.removeObject(forKey: "warm-conversations-home-a-\(repository.lowercased())") }
        let (registry, model) = try await loadWorkingTasks(from: source, repository: repository)
        await spin { await source.preparedTasks.count < SessionsWorkspaceRegistry.warmConversationLimit }
        await spin { registry.preparationCount > 0 }
        let id = Prefetch.unstarted
        await source.hold(["session", "resume"])
        let workspace = registry.open(WorkWorkspaceKey(repository: repository, kind: .task, id: id), title: "Refused", location: nil)
        model.openWarmConversations()
        await source.waitForHeldRead()
        #expect(workspace.conversationIntent == .reveal)

        await source.setPreparationFailure("No provider history", once: true)
        try await source.startTask(id)
        await model.refresh()
        #expect(workspace.conversationIntent == .start)
        await source.release()
        await spin { await !source.preparedTasks.contains(id) }
        await spin { workspace.openingConversation }
        #expect(await source.preparedTasks.contains(id))
        #expect(workspace.conversationError == nil)
        #expect(workspace.conversationStart == nil)
        registry.prepareForQuit()
    }

    @Test("A link's reveal refused after its Task started leaves no offer and starts")
    func supersededLinkRefusalDoesNotLand() async throws {
        let source = try Source()
        let repository = "repo_" + UUID().uuidString
        defer { UserDefaults.standard.removeObject(forKey: "warm-conversations-home-a-\(repository.lowercased())") }
        let (registry, model) = try await loadWorkingTasks(from: source, repository: repository)
        await spin { await source.preparedTasks.count < SessionsWorkspaceRegistry.warmConversationLimit }
        await spin { registry.preparationCount > 0 }
        let id = Prefetch.unstarted
        await source.hold(["session", "resume"])
        let opening = Task { await model.openTaskLink(TaskLink(issue: Prefetch.unstarted.uppercased(), repo: Self.repo, conversation: true)) }
        await source.waitForHeldRead()
        let workspace = try #require(registry.retained(WorkWorkspaceKey(repository: repository, kind: .task, id: id)))
        #expect(workspace.conversationIntent == .reveal)

        await source.setPreparationFailure("No provider history", once: true)
        try await source.startTask(id)
        await model.refresh()
        #expect(workspace.conversationIntent == .start)
        await source.release()
        await opening.value
        await spin { await !source.preparedTasks.contains(id) }
        await spin { registry.preparationCount > 0 }
        #expect(await source.preparedTasks.contains(id))
        #expect(workspace.conversationError == nil)
        #expect(workspace.conversationStart == nil)
        #expect(model.taskOpening == nil)
        registry.prepareForQuit()
    }

    @Test("A board past the bound opens its page; a turn opens the arriving tile and no other")
    func pageTurnOpensArrivingTiles() async throws {
        let source = try Source()
        let repository = "repo_" + UUID().uuidString
        defer { UserDefaults.standard.removeObject(forKey: "warm-conversations-home-a-\(repository.lowercased())") }
        let (registry, model) = try await loadWorkingTasks(from: source, repository: repository)
        let limit = SessionsWorkspaceRegistry.warmConversationLimit
        await spin(for: .seconds(5)) { await source.preparedTasks.count < limit }
        await spin(for: .seconds(5)) { registry.preparationCount > 0 }

        func key(_ id: String) -> WorkWorkspaceKey { WorkWorkspaceKey(repository: repository, kind: .task, id: id) }
        model.applySelectionPlans([fixtureViewPlan(
            owner: .init(repository: repository, kind: .repository, id: repository),
            members: Prefetch.working.map { fixtureViewContent(work: key($0), title: $0) })], reason: "")
        model.selectWorkingTasksDashboard()
        await spin(for: .seconds(5)) { registry.preparationCount > 0 }
        let atLoad = await source.preparedTasks
        #expect(atLoad.count <= limit + 1) // Sixteen and a one-slot page, not the whole board.
        #expect(registry.openWork.count == Prefetch.working.count)

        let tiles = model.workingTaskMembers.map(\.work.id)
        let page = try #require(tiles.firstIndex { !atLoad.contains($0) })
        model.turnPage(.workingTasks, to: page)
        await spin(for: .seconds(5)) { await !source.preparedTasks.contains(tiles[page]) }
        await spin(for: .seconds(5)) { registry.preparationCount > 0 }
        #expect(await source.preparedTasks.count == atLoad.count + 1)
        registry.prepareForQuit()
    }

    @Test("Focusing a tile past the bound starts its conversation, as selection does")
    func focusedOffPageTileStarts() async throws {
        let source = try Source()
        let repository = "repo_" + UUID().uuidString
        defer { UserDefaults.standard.removeObject(forKey: "warm-conversations-home-a-\(repository.lowercased())") }
        let (registry, model) = try await loadWorkingTasks(from: source, repository: repository)
        let limit = SessionsWorkspaceRegistry.warmConversationLimit
        await spin(for: .seconds(5)) { await source.preparedTasks.count < limit }
        await spin(for: .seconds(5)) { registry.preparationCount > 0 }

        func key(_ id: String) -> WorkWorkspaceKey { WorkWorkspaceKey(repository: repository, kind: .task, id: id) }
        model.applySelectionPlans([fixtureViewPlan(
            owner: .init(repository: repository, kind: .repository, id: repository),
            members: Prefetch.working.map { fixtureViewContent(work: key($0), title: $0) })], reason: "")
        model.selectWorkingTasksDashboard()
        await spin(for: .seconds(5)) { registry.preparationCount > 0 }
        let prepared = await source.preparedTasks
        let cold = try #require(model.workingTaskMembers.map(\.work.id).first { !prepared.contains($0) })

        let pane = try #require(registry.retained(key(cold))).multiplexer.focusedPane
        let target = DesktopPaneTarget(repository: key(cold).repository, window: UUID().uuidString, work: key(cold),
                                       pane: pane.id, incarnation: pane.incarnation, tab: nil)
        _ = try DesktopAction.perform(DesktopActionRequest("focus", at: target), model: model, workspaces: registry)
        await spin(for: .seconds(5)) { await !source.preparedTasks.contains(cold) }
        #expect(await source.preparedFlags[cold] == "--start-if-free")
        registry.prepareForQuit()
    }

    @Test("Every working tile on the page opens with no click, two preparations at a time")
    func visibleTilesOpenWithoutSavedVisits() async throws {
        let source = try Source()
        let repository = "repo_" + UUID().uuidString
        defer { UserDefaults.standard.removeObject(forKey: "warm-conversations-home-a-\(repository.lowercased())") }
        let (registry, model) = try await loadWorkingTasks(from: source, repository: repository)
        let limit = SessionsWorkspaceRegistry.warmConversationLimit
        await spin(for: .seconds(5)) { await source.preparedTasks.count < limit }
        await spin(for: .seconds(5)) { registry.preparationCount > 0 }
        #expect(await source.preparedTasks.count == limit) // Warm prefetch stops; five working Tasks remain.

        await source.holdPreparations()
        let working = Prefetch.working, unstarted = Prefetch.unstarted
        func key(_ id: String) -> WorkWorkspaceKey { WorkWorkspaceKey(repository: repository, kind: .task, id: id) }
        let tiles = working + [unstarted]
        model.applySelectionPlans([fixtureViewPlan(
            owner: .init(repository: repository, kind: .repository, id: repository),
            members: tiles.map { fixtureViewContent(work: key($0), title: $0) },
            slots: DashboardSlotLayout(root: .split(.vertical, tiles.map { _ in 1 }, tiles.indices.map { .slot($0) })))], reason: "")
        model.selectWorkingTasksDashboard()
        await spin(for: .seconds(5)) { await source.preparedTasks.count < limit + 2 }
        #expect(registry.preparationCount == 2)
        let prepared = await source.preparedTasks
        let queued = try #require(working.first { !prepared.contains($0) })
        let tile = try #require(registry.retained(key(queued)))
        #expect(registry.visibleWork.contains(tile.key))
        #expect(!tile.openingConversation)
        // The tile shows the pane's placeholder: a queued working Task is opening, never a gate.
        let store = registry.sessionStore(repository: repository, repoPath: Self.repo, query: source.query)
        let offer = ConversationPlaceholder(workspace: tile, model: model, sessions: store)
        #expect(throws: (any Error).self) {
            try offer.inspect().find(viewWithAccessibilityIdentifier: "open-conversation")
        }
        _ = try offer.inspect().find(text: "Opening… Input paused.")

        await source.releasePreparations()
        for id in working {
            let tile = try #require(registry.retained(key(id)))
            await spin(for: .seconds(5)) { tile.multiplexer.pane(forSessionId: "conversation-" + id) == nil }
            #expect(tile.multiplexer.pane(forSessionId: "conversation-" + id) != nil)
            #expect(tile.conversationIntent == .start)
        }
        // An unstarted Task has no conversation to open; its tile offers Start instead.
        let unstartedTile = try #require(registry.retained(key(unstarted)))
        let unstartedTask = try #require(model.task(id: unstarted)?.task)
        #expect(await !source.preparedTasks.contains(unstarted))
        #expect(unstartedTile.conversationError == nil)
        #expect(unstartedTask.awaitsStart)
        _ = try TaskStartOffer(model: model, task: unstartedTask).inspect()
            .find(viewWithAccessibilityIdentifier: "task-start-\(unstartedTask.id)")
        registry.prepareForQuit()
    }

    @Test("A preparation reply behind its Session frame leaves the person's focus alone")
    func latePreparationKeepsFocus() async throws {
        let source = try Source()
        await source.holdPreparations()
        let repository = "repo_" + UUID().uuidString
        defer { UserDefaults.standard.removeObject(forKey: "warm-conversations-home-a-\(repository.lowercased())") }
        let (registry, model) = try await loadWorkingTasks(from: source, repository: repository)
        await spin { await source.preparedTasks.count < 2 }
        let id = try #require(await source.preparedTasks.first)
        let workspace = try #require(registry.retained(WorkWorkspaceKey(repository: repository, kind: .task, id: id)))
        workspace.multiplexer.reveal(sessionId: "conversation-" + id, location: workspace.location)
        workspace.multiplexer.showBrowser(.files)
        let focus = workspace.multiplexer.focusedPaneId
        await source.releasePreparations()
        let store = registry.sessionStore(repository: repository, repoPath: Self.repo, query: source.query)
        await spin { store.sessions.first(where: { $0.id == "conversation-" + id })?.state != .prepared }
        await spin { workspace.openingConversation }
        #expect(store.sessions.first { $0.id == "conversation-" + id }?.state == .prepared)
        #expect(workspace.multiplexer.focusedPaneId == focus)
        registry.prepareForQuit()
    }

    @Test("A Wave named after another Wave's ID keeps its own primary without repeated preparation")
    func collidingWaveNameSettlesBothPrimaries() async throws {
        let source = try Source()
        try await source.addWaveNamedAfterExistingID()
        let repository = "repo_" + UUID().uuidString
        defer { UserDefaults.standard.removeObject(forKey: "warm-conversations-home-a-\(repository.lowercased())") }
        let (registry, model) = try await loadWorkingTasks(from: source, repository: repository)
        defer { registry.prepareForQuit() }
        await spin(for: .seconds(5)) {
            let count = await source.preparedPrimaries.count
            return registry.preparationCount > 0 && count < 8
        }
        let first = WorkWorkspaceKey(repository: repository, kind: .wave, id: "wave-1")
        let second = WorkWorkspaceKey(repository: repository, kind: .wave, id: "wave-2")
        #expect(registry.retained(first)?.multiplexer.sessionPane?.sessionId == "primary-wave:wave-1")
        #expect(registry.retained(second)?.multiplexer.sessionPane?.sessionId == "primary-wave:wave-2")
        #expect(await source.preparedPrimaries == ["repository", "wave:wave-1", "wave:wave-2"])
        // Repeated fresh frames cannot displace either pane and reopen it.
        for _ in 0..<3 { await model.refresh() }
        #expect(await source.preparedPrimaries.count == 3)
        #expect(registry.preparationCount == 0)
    }

    @Test("Repository and Wave primaries open on load beside the Task bound, before any selection")
    func loadOpensRepositoryAndWavePrimaries() async throws {
        let source = try Source()
        let repository = "repo_" + UUID().uuidString
        defer { UserDefaults.standard.removeObject(forKey: "warm-conversations-home-a-\(repository.lowercased())") }
        let (registry, model) = try await loadWorkingTasks(from: source, repository: repository)
        let repositoryKey = WorkWorkspaceKey(repository: repository, kind: .repository, id: repository)
        // The fixture's second Wave belongs to another repository; this window never loads it.
        let wave = WorkWorkspaceKey(repository: repository, kind: .wave, id: "wave-1")
        #expect(registry.warmPrimaries == [repositoryKey, wave])
        #expect(registry.recentTasks.count == SessionsWorkspaceRegistry.warmConversationLimit)
        await spin { await source.preparedPrimaries.count < 2 }
        #expect(await source.preparedPrimaries == ["repository", "wave:wave-1"]) // The selected repository leads.
        let store = registry.sessionStore(repository: repository, repoPath: Self.repo, query: source.query)
        let primaries = ["primary-repository", "primary-wave:wave-1"]
        await spin { store.sessions.filter({ primaries.contains($0.id) && $0.state == .prepared }).count < 2 }
        #expect(store.sessions.filter { primaries.contains($0.id) }.allSatisfy { $0.state == .prepared && $0.launchArguments.contains("--start-if-free") })
        for key in [repositoryKey, wave] {
            let workspace = try #require(registry.retained(key))
            #expect(workspace.conversationIntent == .start)
            #expect(workspace.conversationError == nil)
            #expect(workspace.multiplexer.sessionPane?.sessionId?.hasPrefix("primary-") == true)
        }
        #expect(model.selection == nil)
        registry.prepareForQuit()
    }

    @Test("A queued or failed primary shows opening or its failure with Retry, never an Open gate")
    func primaryPaneNeverOffersAnOpenGate() async throws {
        let source = try Source()
        try await source.populateWorkingTasks(owned: Prefetch.owned)
        await source.setLocationFailure(nil)
        let repository = "repo_" + UUID().uuidString
        defer { UserDefaults.standard.removeObject(forKey: "warm-conversations-home-a-\(repository.lowercased())") }
        let registry = SessionsWorkspaceRegistry(localMachineId: "home-a")
        registry.canPrepare = { false } // Both slots are busy in another window.
        let model = WorkModel(query: source.query, repoPath: Self.repo)
        model.confirmMachine("home-a")
        model.bindWorkspaces(registry, repository: repository)
        await model.refresh()
        let repositoryKey = WorkWorkspaceKey(repository: repository, kind: .repository, id: repository)
        let waveKey = WorkWorkspaceKey(repository: repository, kind: .wave, id: "wave-1")
        let queued = try #require(registry.retained(waveKey))
        #expect(registry.preparationCount == 0)
        #expect(!queued.openingConversation)
        #expect(model.conversationIsOpening(queued))
        let store = registry.sessionStore(repository: repository, repoPath: Self.repo, query: source.query)
        let view = WorkspacePanesView(workspace: queued, isFocused: true, sessions: store, model: model,
            onFocus: { id, _ in queued.multiplexer.setFocusedPane(id) })
        _ = try view.inspect().find(text: "Opening… Input paused.")
        #expect(throws: (any Error).self) { try view.inspect().find(viewWithAccessibilityIdentifier: "open-conversation") }

        await source.setPreparationFailure("Claude account unavailable")
        registry.canPrepare = nil
        registry.continueOpenings()
        let failed = try #require(registry.retained(repositoryKey))
        await spin { failed.conversationError == nil }
        #expect(failed.conversationError == "Claude account unavailable")
        #expect(!model.conversationIsOpening(failed))
        let failedView = WorkspacePanesView(workspace: failed, isFocused: true, sessions: store, model: model,
            onFocus: { id, _ in failed.multiplexer.setFocusedPane(id) })
        _ = try failedView.inspect().find(text: "Claude account unavailable")
        let button = try failedView.inspect().find(viewWithAccessibilityIdentifier: "open-conversation").button()
        #expect(try button.labelView().text().string() == "Retry")
        registry.prepareForQuit()
    }

    private func loadWorkingTasks(from source: Source, repository: String) async throws -> (SessionsWorkspaceRegistry, WorkModel) {
        try await source.populateWorkingTasks(owned: Prefetch.owned)
        await source.setLocationFailure(nil)
        let registry = SessionsWorkspaceRegistry(localMachineId: "home-a")
        let model = WorkModel(query: source.query, repoPath: Self.repo)
        model.confirmMachine("home-a")
        model.bindWorkspaces(registry, repository: repository)
        await model.refresh()
        return (registry, model)
    }

    @Test("Saved text never claims a live process or a legal mutation")
    func savedTextIsQuiet() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let saving = WorkCache(directory: directory)
        let first = WorkModel(query: source.query, repoPath: Self.repo, cache: saving)
        await first.refresh()
        let liveTask = try #require(first.task(id: "issue-now")?.task)
        let live = try #require(liveTask.latestGraphProcess)
        #expect(liveTask.execution?.state == .running)
        #expect(first.sessions.value?.first?.state == .active)
        #expect(first.task(id: "issue-now")?.task.condition.reason != WorkCache.savedReason)
        saving.flush()

        let returning = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("offline") },
                                    repoPath: Self.repo, cache: WorkCache(directory: directory))
        let task = try #require(returning.task(id: "issue-now")?.task)
        let latest = try #require(task.latestGraphProcess)
        #expect(task.execution?.state == .unknown)
        #expect(latest.current == live.current)
        #expect(task.runControl.unavailable == WorkCache.savedReason)
        let conditions = returning.roadmap.value?.waves.flatMap { $0.tasks.items.map(\.condition) } ?? []
        #expect(!conditions.isEmpty)
        #expect(conditions.allSatisfy { $0.state == .unknown && $0.reason == WorkCache.savedReason })
        let session = try #require(returning.sessions.value?.first)
        #expect(session.state == .unknown)
        #expect(session.actions.map(\.kind) == [.open])
        returning.select(.task(id: task.id))
        returning.navigation.selectedSessionId = session.id
        let inspection = returning.inspectDesktop(repository: "plan", window: UUID(), workspaces: SessionsWorkspaceRegistry())
        #expect(inspection.task?.reading == "saved")
        #expect(inspection.task?.actions == nil)
        #expect(inspection.task?.runControl == nil)
        #expect(inspection.session?.reading == "saved")
        #expect(inspection.session?.actions == nil)
    }

    @Test("A failed refresh keeps the saved workspace under one truthful status")
    func failedRefreshKeepsSaved() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let saving = WorkCache(directory: directory)
        await WorkModel(query: source.query, repoPath: Self.repo, cache: saving).refresh()
        saving.flush()

        await source.fail()
        let returning = WorkModel(query: source.query, repoPath: Self.repo, cache: WorkCache(directory: directory))
        await returning.refresh()

        #expect(returning.workStatus == .failed("Couldn't update: offline"))
        #expect(returning.roadmap.value != nil)
        #expect(returning.sessions.value?.isEmpty == false)
        let navigator = WorkNavigator(model: returning, onOpenSession: { _ in })
        let status = try navigator.inspect().findAll(where: { (try? $0.accessibilityIdentifier()) == "work-status" })
        #expect(status.count == 1)

        await source.recover()
        await returning.refresh()
        #expect(returning.workStatus == .current)
        guard let execution = returning.task(id: "issue-now")?.task.execution else {
            Issue.record("fresh Graph has no latest record"); return
        }
        #expect(execution.state == .running)
    }

    @Test("A first launch has one loading message")
    func firstLaunchLoadsOnce() throws {
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("unused") },
                                repoPath: Self.repo, cache: WorkCache(directory: try temporaryDirectory()))
        let navigator = WorkNavigator(model: model, onOpenSession: { _ in })
        let texts = try navigator.inspect().findAll(ViewType.Text.self).map { try $0.string() }
        #expect(texts.filter { $0.contains("…") } == ["Loading work…"])
    }

    @Test("Absent, corrupt and incompatible files load as nothing and are removed")
    func unusableFilesRecover() throws {
        let directory = try temporaryDirectory()
        let file = directory.appendingPathComponent("workspace.json")
        #expect(WorkCache(directory: directory).load() == nil)

        for text in ["{ not json", #"{"version":0,"snapshot":{"repositories":{}}}"#,
                     #"{"version":1,"snapshot":{"homeId":"home-a","repositories":{}}}"#] {
            try Data(text.utf8).write(to: file)
            #expect(WorkCache(directory: directory).load() == nil)
            #expect(!FileManager.default.fileExists(atPath: file.path))
        }

        try Data(#"{"version":1,"snapshot":{"roadmap":"{}","repositories":{}}}"#.utf8).write(to: file)
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("unused") },
                                repoPath: Self.repo, cache: WorkCache(directory: directory))
        #expect(model.workStatus == .loading)
    }

    @Test("A workspace saved under another Machine is dropped")
    func anotherMachineInvalidates() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let saving = WorkCache(directory: directory)
        let first = WorkModel(query: source.query, repoPath: Self.repo, cache: saving)
        first.confirmMachine("home-a")
        await first.refresh()
        first.select(.task(id: "issue-now"))
        saving.flush()

        let cache = WorkCache(directory: directory)
        let returning = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("offline") },
                                    repoPath: Self.repo, cache: cache)
        returning.confirmMachine("home-b")
        cache.flush()

        #expect(returning.workStatus == .loading)
        #expect(returning.sessions.value == nil)
        #expect(returning.selection == nil)
        #expect(returning.task(id: "issue-now") == nil)
        let saved = try #require(WorkCache(directory: directory).load())
        #expect(saved == WorkSnapshot(machineId: "home-b"))
    }

    @Test("A Session read that finishes after a repository switch saves nothing")
    func staleSessionReadIsNotSaved() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let cache = WorkCache(directory: directory)
        let model = WorkModel(query: source.query, repoPath: Self.repo, cache: cache)
        await source.hold(["session", "list"])
        let read = Task { await model.refreshSessions() }
        await source.waitForHeldRead()
        model.setRepoPath("/src/other")
        await source.release()
        await read.value
        cache.flush()

        #expect(model.sessions.value == nil)
        #expect(WorkCache(directory: directory).load()?.repositories[Self.repo]?.sessionPages == nil)
    }

    @Test("A slow provider keeps the saved workspace usable under Updating…")
    func slowProviderKeepsSaved() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let saving = WorkCache(directory: directory)
        await WorkModel(query: source.query, repoPath: Self.repo, cache: saving).refresh()
        saving.flush()

        let returning = WorkModel(query: source.query, repoPath: Self.repo, cache: WorkCache(directory: directory))
        await source.hold(["plan", "list"])
        let read = Task { await returning.refresh() }
        await source.waitForHeldRead()

        #expect(returning.workStatus == .updating)
        #expect(returning.task(id: "issue-now") != nil)
        #expect(returning.sessions.value?.isEmpty == false)
        let navigator = WorkNavigator(model: returning, onOpenSession: { _ in })
        let texts = try navigator.inspect().findAll(ViewType.Text.self).map { try $0.string() }
        #expect(texts.filter { $0.contains("…") } == ["Updating…"])

        await source.release()
        await read.value
        #expect(returning.workStatus == .current)
    }

    @Test("A refresh that finishes after a selection change keeps the newer selection")
    func refreshAfterSelectionChange() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let saving = WorkCache(directory: directory)
        let first = WorkModel(query: source.query, repoPath: Self.repo, cache: saving)
        await first.refresh()
        first.select(.task(id: "issue-now"))
        saving.flush()

        let cache = WorkCache(directory: directory)
        let returning = WorkModel(query: source.query, repoPath: Self.repo, cache: cache)
        #expect(returning.selection == .task(id: "issue-now"))
        await source.hold(["plan", "list"])
        let read = Task { await returning.refresh() }
        await source.waitForHeldRead()
        returning.select(.wave(id: "wave-1"))
        await source.release()
        await read.value
        cache.flush()

        #expect(returning.selection == .wave(id: "wave-1"))
        #expect(returning.navigation.content == .details)
        #expect(returning.workStatus == .current)
        #expect(WorkCache(directory: directory).load()?.repositories[Self.repo]?.selection == .wave(id: "wave-1"))
    }

    @Test("Another window in the same process opens from the workspace already held")
    func warmReopen() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let cache = WorkCache(directory: directory)
        let first = WorkModel(query: source.query, repoPath: Self.repo, cache: cache)
        await first.refresh()
        cache.flush()
        try FileManager.default.removeItem(at: directory.appendingPathComponent("workspace.json"))

        let reads = await source.reads
        let reopened = WorkModel(query: source.query, repoPath: Self.repo, cache: cache)

        #expect(reopened.workStatus == .updating)
        #expect(reopened.roadmap.value?.waves.map(\.wave.id) == first.roadmap.value?.waves.map(\.wave.id))
        #expect(reopened.sessions.value?.map(\.id) == first.sessions.value?.map(\.id))
        #expect(await source.reads == reads)
    }

    @Test("A saved launch repository opens without git and is checked afterwards")
    func savedLaunchRepository() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let saving = WorkCache(directory: directory)
        await WorkModel(query: source.query, repoPath: Self.repo, cache: saving).refresh()
        saving.flush()

        // `/src/loopflow` is no repository here, so only the saved workspace can scope the window.
        let unknown = WorkModel(query: source.query, launchCandidates: [Self.repo],
                                  cache: WorkCache(directory: try temporaryDirectory()))
        #expect(unknown.repoPath == nil)
        let returning = WorkModel(query: source.query, launchCandidates: ["/src/missing", Self.repo],
                                    cache: WorkCache(directory: directory))
        #expect(returning.repoPath == Self.repo)
        #expect(returning.sessions.value?.isEmpty == false)

        await returning.applyPortfolio([], initialRepoPath: nil)
        #expect(returning.repoPath == nil)
    }

    @Test("Only the most recently saved repositories are kept")
    func repositoriesAreBounded() throws {
        let directory = try temporaryDirectory()
        let cache = WorkCache(directory: directory)
        let start = Date(timeIntervalSince1970: 1_000)
        for index in 0..<(WorkCache.maxRepositories + 3) {
            cache.saveSelection(.wave(id: "wave"), repo: "/src/\(index)", at: start.addingTimeInterval(Double(index)))
        }
        cache.flush()

        let saved = try #require(WorkCache(directory: directory).load())
        #expect(saved.repositories.count == WorkCache.maxRepositories)
        #expect(saved.repositories["/src/0"] == nil)
        #expect(saved.repositories["/src/\(WorkCache.maxRepositories + 2)"] != nil)
    }

    @Test("An unchanged poll writes nothing")
    func unchangedPollDoesNotRewrite() async throws {
        let directory = try temporaryDirectory()
        let source = try Source()
        let cache = WorkCache(directory: directory)
        let model = WorkModel(query: source.query, repoPath: Self.repo, cache: cache)
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
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("work-cache-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }
}

private extension SessionsStore {
    /// Task conversations lf prepared, apart from the repository and Wave primaries a load also opens.
    var preparedTaskConversations: [SessionItem] { sessions.filter { $0.state == .prepared && $0.record.taskPrimary } }
}

/// `populateWorkingTasks` rows: `owned` working Tasks, oldest first, then shipping,
/// done, abandoned and unstarted Tasks, then a started Task nobody owns.
@MainActor
private enum Prefetch {
    /// Five more working Tasks than the warm bound.
    static let owned = SessionsWorkspaceRegistry.warmConversationLimit + 4
    static let newestOwned = "prefetch-\(owned - 1)"
    static let unstarted = "prefetch-\(owned + 3)"
    static let unowned = "prefetch-\(owned + 4)"
    static let working = (0..<owned).map { "prefetch-\($0)" } + [unowned]
}

private actor Source {
    private var roadmap: String
    private let sessions: String
    private var failed = false
    private var locationFailure: String? = "Location unavailable"
    private var preparationFailure: String?
    private var failsOnce = false
    private(set) var preparedTasks: [String] = []
    private(set) var preparedFlags: [String: String] = [:]
    /// Repository and Wave primaries prepared on load: "repository" or "wave:<id>".
    private(set) var preparedPrimaries: [String] = []
    private var holding: [String]?
    private(set) var reads = 0
    private var held: CheckedContinuation<Void, Never>?
    private var arrived: CheckedContinuation<Void, Never>?
    private var preparationsHeld = false
    private var preparationWaiters: [CheckedContinuation<Void, Never>] = []

    init() throws {
        let fixtures = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto")
        roadmap = try String(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json"), encoding: .utf8)
        sessions = try String(contentsOf: fixtures.appendingPathComponent("session_page.json"), encoding: .utf8)
    }

    nonisolated var query: RegistryQuery { RegistryQuery { args, _ in try await self.read(args) } }

    func holdPreparations() { preparationsHeld = true }

    func releasePreparations() {
        preparationsHeld = false
        let waiters = preparationWaiters
        preparationWaiters = []
        for waiter in waiters { waiter.resume() }
    }

    func addWaveNamedAfterExistingID() throws {
        var wire = try JSONSerialization.jsonObject(with: Data(roadmap.utf8)) as! [String: Any]
        var waves = wire["waves"] as! [[String: Any]]
        var duplicate = waves[0]
        var wave = duplicate["wave"] as! [String: Any]
        wave["name"] = wave["id"]
        wave["id"] = "wave-2"
        duplicate["wave"] = wave
        waves.append(duplicate)
        wire["waves"] = waves
        roadmap = String(decoding: try JSONSerialization.data(withJSONObject: wire), as: UTF8.self)
    }

    func populateWorkingTasks(owned: Int) throws {
        var wire = try JSONSerialization.jsonObject(with: Data(roadmap.utf8)) as! [String: Any]
        var inventory = wire["tasks"] as! [String: Any]
        let template = (inventory["items"] as! [[String: Any]])[0]
        // The newest row is a started Task nobody owns; it opens like any working Task.
        let states = (0..<owned).map { ["idle", "active", "moving"][$0 % 3] } + ["shipping", "done", "abandoned", "unstarted", "idle"]
        inventory["items"] = states.enumerated().map { index, state in
            var row = template
            var task = row["task"] as! [String: Any]
            task["id"] = "prefetch-\(index)"
            task["identifier"] = "PREFETCH-\(index)"
            row["task"] = task
            var runtime = row["runtime"] as! [String: Any]
            runtime["work_id"] = "prefetch-\(index)"
            runtime["status"] = state
            runtime["updated_at"] = String(format: "2026-11-04T10:00:%02dZ", index)
            runtime["owner"] = index == states.count - 1 ? NSNull() : [
                "person": "Jack Heart", "person_key": "jack@loopflow.studio",
                "machine": ["id": "home-a", "label": "laptop"]
            ] as [String: Any]
            row["runtime"] = runtime
            return row
        }
        wire["tasks"] = inventory
        roadmap = String(decoding: try JSONSerialization.data(withJSONObject: wire), as: UTF8.self)
    }

    func setLocationFailure(_ reason: String?) { locationFailure = reason }
    func setPreparationFailure(_ reason: String?, once: Bool = false) {
        preparationFailure = reason
        failsOnce = once
    }

    /// The Task's checkout was placed: it is now idle, a working Task.
    func startTask(_ id: String) throws {
        var wire = try JSONSerialization.jsonObject(with: Data(roadmap.utf8)) as! [String: Any]
        var inventory = wire["tasks"] as! [String: Any]
        inventory["items"] = (inventory["items"] as! [[String: Any]]).map { item in
            guard (item["task"] as? [String: Any])?["id"] as? String == id else { return item }
            var item = item
            var runtime = item["runtime"] as! [String: Any]
            runtime["status"] = "idle"
            item["runtime"] = runtime
            return item
        }
        wire["tasks"] = inventory
        roadmap = String(decoding: try JSONSerialization.data(withJSONObject: wire), as: UTF8.self)
    }

    func completeTask(_ id: String) throws {
        var wire = try JSONSerialization.jsonObject(with: Data(roadmap.utf8)) as! [String: Any]
        func completed(_ inventory: [String: Any]) -> [String: Any] {
            var inventory = inventory
            inventory["items"] = (inventory["items"] as! [[String: Any]]).map { item in
                guard (item["task"] as? [String: Any])?["id"] as? String == id else { return item }
                var item = item
                var runtime = item["runtime"] as! [String: Any]
                runtime["status"] = "done"
                item["runtime"] = runtime
                return item
            }
            return inventory
        }
        wire["tasks"] = completed(wire["tasks"] as! [String: Any])
        roadmap = String(decoding: try JSONSerialization.data(withJSONObject: wire), as: UTF8.self)
    }

    func fail() { failed = true }
    func recover() { failed = false }
    func hold(_ command: [String]) { holding = command }

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
        var command = args.first == "--machine" ? Array(args.dropFirst(4)) : args
        if command.first == "--repo" { command = Array(command.dropFirst(2)) }
        if let holding, command.starts(with: holding) {
            await withCheckedContinuation { continuation in
                held = continuation
                arrived?.resume()
                arrived = nil
            }
        }
        if command.starts(with: ["task", "location"]) {
            if let locationFailure, locationFailure != "Recorded checkout changed" { throw RegistryQueryError(locationFailure) }
            let snapshot = try RegistryQuery.decode(RoadmapSnapshot.self, from: roadmap)
            let task = try #require(snapshot.tasks.items.first { $0.runtime?.workId == command[2] })
            let location = try #require(task.reference.workspace?.identity)
            let request = command[try #require(command.firstIndex(of: "--request")) + 1]
            return String(decoding: try JSONEncoder().encode([TaskLocationObservation(request: request,
                repositoryID: args[3], taskID: command[2], machineID: location.machineId, observedAt: 1,
                location: .recorded(taskID: command[2], checkout: location.worktree + (locationFailure == nil ? "" : "/moved")))]), as: UTF8.self)
        }
        if command.starts(with: ["session", "resume"]), command.contains("--explain") { return taskConversationStartOfferFixture() }
        if command.starts(with: ["session", "resume"]), let preparationFailure {
            if failsOnce { self.preparationFailure = nil }
            throw RegistryQueryError(preparationFailure)
        }
        let openFlag = ["--start-if-free", "--reveal-existing"].first(where: command.contains)
        if command.starts(with: ["session", "resume"]), let openFlag, !command.contains("--task") {
            // A repository or Wave primary: Rust places it in the scope's own checkout.
            let wave = command.firstIndex(of: "--wave").map { command[$0 + 1] }
            let snapshot = try RegistryQuery.decode(RoadmapSnapshot.self, from: roadmap)
            let waveId = try wave.map { name in try #require(snapshot.waves.first { $0.wave.id == name } ?? snapshot.waves.first { $0.wave.name == name }).wave.id }
            let scope = waveId.map { "wave:" + $0 } ?? "repository"
            let page = try JSONSerialization.jsonObject(with: Data(sessions.utf8)) as! [String: Any]
            var record = (page["entries"] as! [[String: Any]])[0]
            record["id"] = "primary-" + scope
            record["primary_scope"] = wave == nil ? "repository" : "wave"
            record["wave_id"] = waveId ?? NSNull()
            record["task_ids"] = []
            record["task_primary"] = false
            record["workspace"] = ["machine_id": "home-a", "worktree": "/src/loopflow." + (wave ?? "repo"), "task_id": NSNull()]
            record["state"] = "closed"
            record["actions"] = sessionActionFixture(state: "closed")
            record["open_argv"] = ["lf", "session", "resume", "primary-" + scope, openFlag]
            preparedPrimaries.append(scope)
            return String(decoding: try JSONSerialization.data(withJSONObject: record), as: UTF8.self)
        }
        if command.starts(with: ["session", "resume"]), let openFlag {
            let id = command[try #require(command.firstIndex(of: "--task")) + 1]
            let snapshot = try RegistryQuery.decode(RoadmapSnapshot.self, from: roadmap)
            let task = try #require(snapshot.tasks.items.first { $0.runtime?.workId == id })
            let location = try #require(task.reference.workspace?.identity)
            let page = try JSONSerialization.jsonObject(with: Data(sessions.utf8)) as! [String: Any]
            var record = (page["entries"] as! [[String: Any]])[0]
            record["id"] = "conversation-" + id
            record["task_ids"] = [id]
            record["task_primary"] = true
            record["workspace"] = ["machine_id": location.machineId, "worktree": location.worktree, "task_id": id]
            record["state"] = "closed"
            record["actions"] = sessionActionFixture(state: "closed")
            record["open_argv"] = ["lf", "session", "resume", "conversation-" + id, openFlag]
            preparedTasks.append(id)
            preparedFlags[id] = openFlag
            if preparationsHeld {
                await withCheckedContinuation { preparationWaiters.append($0) }
            }
            return String(decoding: try JSONSerialization.data(withJSONObject: record), as: UTF8.self)
        }
        switch command.first {
        case "plan" where command.dropFirst().first == "list": return roadmap
        case "session" where command.dropFirst().first == "list": return sessions
        case "wave": return "[]"
        case "session" where command.dropFirst().first == "history": return #"{"truncated":false,"items":[]}"#
        default: throw RegistryQueryError("unexpected command")
        }
    }
}
/// Yield while `condition` holds, for at most `timeout`; the assertion follows.
@MainActor
private func spin(for timeout: Duration = .seconds(3), while condition: @MainActor () async -> Bool) async {
    let deadline = ContinuousClock.now + timeout
    while await condition(), ContinuousClock.now < deadline { await Task.yield() }
}

#endif
