#if os(macOS)
import Foundation
import Testing
import os
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Sessions store")
@MainActor
struct SessionsStoreTests {
    @Test("Prepared SSH panes use the server’s local relay command")
    func sshPaneUsesPreparedCommand() throws {
        var record = try #require(records([session(id: "remote", state: "closed")]).first)
        let ssh = ["--", "ssh", "peer", "lf session attach remote"]
        record.openArgv = ["/bin/lf", "session", "attach"] + ssh
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { _, _ in "" })
        store.reconcile([record])
        #expect(item(store, "remote")?.launchArguments == record.openArgv)
    }

    @Test("Provider switching keeps the Session and prepares an explicit provider launch", arguments: ["claude", "codex", "opencode", "pi", "agy"])
    func switchingProviderKeepsSession(agent: String) async throws {
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { _, _ in
            session(id: "main", state: "closed")
        })
        store.reconcile(try records([session(id: "main", state: "active"), session(id: "other", state: "available")]))
        let previous = try #require(item(store, "main"))
        try await store.switchProvider("main", to: agent)
        let prepared = try #require(item(store, "main"))
        #expect(prepared.state == .prepared)
        #expect(prepared.id == previous.id)
        #expect(prepared.record.title == previous.record.title)
        #expect(prepared.launchId != previous.launchId)
        #expect(prepared.launchArguments == ["lf", "session", "resume", "main", "--agent", agent])
        #expect(prepared.presentationProvider == agent)
        #expect(item(store, "other")?.state == .pending)
    }

    @Test("Prepared Sessions accept current provider and terminal evidence without losing their launch")
    func preparationRetainsCommandButRefreshesProvider() async throws {
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { _, _ in
            session(id: "main", state: "closed")
        })
        store.reconcile(try records([session(id: "main", state: "active")]))
        try await store.switchProvider("main", to: "claude")
        let launch = try #require(item(store, "main")).launchArguments
        var wire = try #require(JSONSerialization.jsonObject(with: Data(session(id: "main", state: "active").utf8)) as? [String: Any])
        wire["provider"] = "claude"
        wire["agent_process_id"] = "new-agent"
        wire["terminal_ids"] = ["current-terminal"]
        let observed = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: wire))
        store.reconcile([observed])
        let prepared = try #require(item(store, "main"))
        #expect(prepared.record.provider == "claude")
        #expect(prepared.record.agentProcessId == AgentProcessId(rawValue: "new-agent"))
        #expect(prepared.launchArguments == launch)
        #expect(SessionProviderReading(record: prepared.record, terminal: "current-terminal")?.agentProcessId == AgentProcessId(rawValue: "new-agent"))
        #expect(SessionProviderReading(record: prepared.record, terminal: "previous-terminal") == nil)
        store.recordPaneLive("main")
        store.reconcile([observed])
        #expect(item(store, "main")?.agentOverride == nil)
        #expect(item(store, "main")?.presentationProvider == "claude")
    }

    @Test("A refused provider change preserves the original Session presentation")
    func refusedProviderChangeKeepsSession() async throws {
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { _, _ in
            throw RegistryQueryError("Provider unavailable")
        })
        store.reconcile(try records([session(id: "main", state: "active")]))
        let previous = item(store, "main")
        await #expect(throws: RegistryQueryError.self) { try await store.switchProvider("main", to: "claude") }
        #expect(item(store, "main") == previous)
    }

    @Test("A failed transfer retains concurrent Session updates and can retry")
    func failedTransferCanRetry() async throws {
        let (requests, request) = AsyncStream<Void>.makeStream()
        let (responses, response) = AsyncStream<Bool>.makeStream()
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { args, _ in
            guard args.starts(with: ["--agent", "codex"]) else {
                throw RegistryQueryError("Transfer preparation did not select Codex")
            }
            request.yield(())
            for await fails in responses {
                if fails { throw RegistryQueryError("Claude transcript was not found. Retry after Claude saves its first turn.") }
                break
            }
            return session(id: "main", state: "closed")
        })
        let original = try #require(records([session(id: "main", state: "closed")]).first)
        store.reconcile([original])
        store.adoptPreparation(original)
        store.recordPaneLive("main")
        let previous = try #require(item(store, "main"))
        let transfer = Task { try await store.switchProvider("main", to: "codex") }
        for await _ in requests { break }
        #expect(item(store, "main") == previous)
        let updated = try #require(records([session(id: "main", state: "active")
            .replacingOccurrences(of: "Design the control surface", with: "Retained conversation")]).first)
        store.reconcile([updated])
        response.yield(true)
        await #expect(throws: RegistryQueryError.self) { try await transfer.value }
        #expect(item(store, "main")?.record.title == "Retained conversation")
        #expect(item(store, "main")?.launchId == previous.launchId)
        #expect(item(store, "main")?.agentOverride == nil)
        response.yield(false)
        try await store.switchProvider("main", to: "codex")
        #expect(item(store, "main")?.id == original.id)
        #expect(item(store, "main")?.state == .prepared)
        #expect(item(store, "main")?.presentationProvider == "codex")
    }

    @Test("Refresh replaces the viewer and opens the same conversation without resetting it")
    func refreshSameConversation() async throws {
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { args, _ in
            guard args == ["session", "resume", "main", "--prepare", "--json"] else {
                throw RegistryQueryError("unexpected argv: \(args)")
            }
            return session(id: "main", state: "closed", primary: true)
        })
        let original = try #require(records([session(id: "main", state: "active", primary: true)]).first)
        store.reconcile([original])
        store.adoptPreparation(original)
        store.recordPaneLive("main")
        let previous = try #require(item(store, "main"))
        await store.refresh("main")
        #expect(item(store, "main")?.state == .prepared)
        #expect(item(store, "main")?.launchId != previous.launchId)
        #expect(item(store, "main")?.record.id == original.id)
        #expect(item(store, "main")?.agentOverride == nil)
        store.recordPaneFailure(previous, reason: "Retired viewer failed")
        #expect(item(store, "main")?.state == .prepared)
        store.recordPaneFailure(try #require(item(store, "main")), reason: "Current viewer failed")
        #expect(item(store, "main")?.state == .failed("Current viewer failed"))
    }

    @Test("A Task workspace reopens its Session by id with the Task beside it, so it resumes managed")
    func taskWorkspaceReopenNamesTheTask() async throws {
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { args, _ in
            guard args == ["session", "resume", "main", "--task", "task_1", "--prepare", "--json"] else {
                throw RegistryQueryError("unexpected argv: \(args)")
            }
            return session(id: "main", state: "closed")
        }, work: .task(id: "task_1"))
        let original = try #require(records([session(id: "main", state: "active")]).first)
        store.reconcile([original])
        store.adoptPreparation(original)
        store.recordPaneLive("main")
        await store.refresh("main")
        #expect(item(store, "main")?.state == .prepared)
    }

    @Test("Reset restarts the same Session with a fresh launch")
    func resetRestartsSameSession() async throws {
        let submitted = OSAllocatedUnfairLock<String?>(initialState: nil)
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { args, _ in
            guard args.count == 7, Array(args.prefix(4)) == ["session", "new", "main", "--request-id"],
                  Array(args.suffix(2)) == ["--prepare", "--json"] else {
                throw RegistryQueryError("unexpected argv: \(args)")
            }
            submitted.withLock { $0 = args[4] }
            return session(id: "main", state: "closed", primary: true)
        })
        store.reconcile(try records([session(id: "main", state: "active", primary: true), session(id: "other", state: "available")]))
        let previous = try #require(item(store, "main"))
        let restarted = try await store.reset("main")
        #expect(restarted.id == "main")
        #expect(item(store, "main")?.state == .prepared)
        #expect(item(store, "main")?.launchId != previous.launchId)
        #expect(item(store, "main")?.launchArguments == ["lf", "session", "resume", "main"])
        #expect(item(store, "other")?.state == .pending)
        let requestID = try #require(submitted.withLock { $0 })
        #expect(CreationRecovery.shared.requests(for: .session("main")).contains(requestID))
        await CreationRecovery.shared.presented([requestID])
    }

    @Test("Reset leaves the Session live while preparing and keeps updates when it fails", arguments: [false, true])
    func resetPreservesConcurrentUpdates(fails: Bool) async throws {
        let (requests, request) = AsyncStream<Void>.makeStream()
        let (responses, response) = AsyncStream<Void>.makeStream()
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { _, _ in
            request.yield(())
            for await _ in responses { break }
            if fails { throw RegistryQueryError("Preparation failed") }
            return session(id: "main", state: "closed", primary: true)
        })
        store.reconcile(try records([session(id: "main", state: "closed", primary: true)]))
        let previous = try #require(item(store, "main"))
        store.adoptPreparation(previous.record)
        store.recordPaneLive("main")

        let reset = Task { try await store.reset("main") }
        for await _ in requests { break }
        #expect(item(store, "main")?.state == .live)
        store.noteSurfaceClosed(.session("main", machineId: fixtureMachineId), output: "Client exited")
        response.yield(())
        if fails {
            await #expect(throws: RegistryQueryError.self) { try await reset.value }
            #expect(item(store, "main")?.state == .ended("Client exited"))
        } else {
            #expect(try await reset.value.id == "main")
            #expect(item(store, "main")?.state == .prepared)
        }
    }

    @Test("Filtered refresh retains Sessions without creating local terminals")
    func filteredRefreshRetainsSessions() throws {
        let store = SessionsStore(repoPath: "/tmp/repo")
        store.reconcile(try records([
            session(id: "a", state: "available"),
            session(id: "b", state: "active"),
        ]))

        #expect(item(store, "a")?.state == .pending)
        #expect(item(store, "b")?.surface == nil)

        store.reconcile(try records([session(id: "a", state: "available")]))

        #expect(store.sessions.map(\.id) == ["a", "b"])
        #expect(item(store, "a")?.state == .pending)
    }

    @Test("Opening one Session prepares only its terminal")
    func startsOnlyTheSelectedSession() async throws {
        let store = SessionsStore(
            repoPath: "/tmp/repo",
            query: RegistryQuery { args, _ in
                #expect(args == ["session", "resume", "second", "--prepare", "--json"])
                return session(id: "second", state: "available")
            }
        )
        store.reconcile(try records([
            session(id: "first", state: "available"),
            session(id: "second", state: "available"),
        ]))

        await store.select("second")

        #expect(item(store, "first")?.state == .pending)
        #expect(item(store, "second")?.surface?.openArgv.suffix(3) == ["session", "resume", "second"])
    }

    @Test("A prepared Task target enters its terminal without another preparation", arguments: [false, true])
    func taskPreparationIsConsumedDirectly(ended: Bool) async throws {
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { _, _ in
            throw RegistryQueryError("The prepared command is already available")
        })
        let prepared = try #require(records([session(id: "primary", state: "closed")]).first)
        store.reconcile([prepared])
        if ended {
            store.adoptPreparation(prepared)
            store.recordPaneLive(prepared.id)
            store.noteSurfaceClosed(.session(prepared.id, machineId: fixtureMachineId), output: "Provider exited")
            #expect(item(store, prepared.id)?.state == .ended("Provider exited"))
        }
        store.adoptPreparation(prepared)

        await store.select(prepared.id)

        #expect(item(store, prepared.id)?.state == .prepared)
        #expect(item(store, prepared.id)?.surface?.openArgv == prepared.openArgv)
        #expect(item(store, prepared.id)?.error == nil)
    }

    @Test("Opening a Session active in another client takes it over here")
    func openingTakesOverAnotherClient() async throws {
        let store = SessionsStore(
            repoPath: "/tmp/repo",
            query: RegistryQuery { args, _ in
                #expect(args == ["session", "resume", "native", "--prepare", "--json"])
                return session(id: "native", state: "closed")
            }
        )
        store.reconcile(try records([
            session(id: "native", state: "active"),
        ]))

        #expect(item(store, "native")?.state == .pending)
        #expect(item(store, "native")?.record.action?.label == "Take over")

        await store.select("native")

        #expect(item(store, "native")?.state == .prepared)
        #expect(item(store, "native")?.surface?.openArgv == ["lf", "session", "resume", "native"])
    }

    @Test("Polling preserves the prepared interactive launch command", arguments: [false, true])
    func reconcilePreservesPreparedLaunch(replacing: Bool) async throws {
        let store = SessionsStore(
            repoPath: "/tmp/repo",
            query: RegistryQuery { _, _ in
                session(id: "native", state: "closed")
            }
        )
        store.reconcile(try records([
            session(id: "native", state: replacing ? "active" : "closed"),
        ]))

        await store.select("native")
        let prepared = try #require(item(store, "native")?.surface)
        #expect(prepared.openArgv == ["lf", "session", "resume", "native"])
        store.reconcile(try records([
            session(id: "native", state: "active"),
        ]))

        #expect(item(store, "native")?.state == .prepared)
        #expect(item(store, "native")?.surface?.openArgv == prepared.openArgv)
        store.reconcile([])
        store.reconcile([])
        #expect(item(store, "native")?.state == .prepared)
        #expect(item(store, "native")?.surface?.openArgv == prepared.openArgv)
    }

    @Test("An externally killed terminal ends its view here")
    func externalKillEndsTheView() async throws {
        let store = SessionsStore(
            repoPath: "/tmp/repo",
            query: RegistryQuery { _, _ in
                session(id: "native", state: "active")
            }
        )
        store.reconcile(try records([
            session(id: "native", state: "active"),
        ]))
        await store.select("native")
        store.recordPaneLive("native")
        #expect(item(store, "native")?.state == .live)

        store.noteSurfaceClosed(.shell("native", machineId: fixtureMachineId))
        #expect(item(store, "native")?.state == .live)
        store.noteSurfaceClosed(.session("native", machineId: "peer"))
        #expect(item(store, "native")?.state == .live)

        store.noteSurfaceClosed(.session("native", machineId: fixtureMachineId))

        #expect(item(store, "native")?.state == .ended("Terminal output is unavailable."))
        #expect(item(store, "native")?.surface == nil)
    }

    @Test("Ended Session output survives a fresh inventory without relaunch")
    func endedOutputSurvivesPolling() async throws {
        let record = session(id: "native", state: "closed")
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { _, _ in record })
        store.reconcile(try records([record]))
        await store.select("native")
        store.recordPaneLive("native")
        store.noteSurfaceClosed(.session("native", machineId: fixtureMachineId),
            output: "Error: Previous AgentProcess has no observed exit")
        store.reconcile(try records([record]))
        #expect(item(store, "native")?.state == .ended("Error: Previous AgentProcess has no observed exit"))
        #expect(item(store, "native")?.surface == nil)
        await store.select("native")
        #expect(item(store, "native")?.state == .prepared)
    }

    @Test("A failed open stays visible across polling until superseded")
    func failedOpenSurvivesPolling() async throws {
        let store = SessionsStore(
            repoPath: "/tmp/repo",
            query: RegistryQuery { args, _ in
                #expect(args.contains("resume"))
                return "not json"
            }
        )
        store.reconcile(try records([
            session(id: "native", state: "closed"),
        ]))

        await store.select("native")
        #expect(item(store, "native")?.error != nil)

        store.reconcile(try records([
            session(id: "native", state: "closed"),
        ]))
        #expect(item(store, "native")?.error != nil)

        store.reconcile(try records([
            session(id: "native", state: "active"),
        ]))
        #expect(item(store, "native")?.error != nil)
    }


    @Test("An available Session stays listed")
    func availableSessionStaysListed() throws {
        let store = SessionsStore(repoPath: "/tmp/repo")
        store.reconcile(try records([session(id: "review", state: "available")]))

        #expect(store.sessions.map(\.id) == ["review"])
        #expect(store.sessions.first?.record.availability == .available)
    }


    @Test("Session actions use the main repository for a linked worktree")
    func sessionStoreUsesMainRepository() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("session-scope-\(UUID().uuidString)", isDirectory: true)
        let main = root.appendingPathComponent("loopflow", isDirectory: true)
        let worktree = root.appendingPathComponent("loopflow.feature", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try FileManager.default.createDirectory(at: main, withIntermediateDirectories: true)
        try runGit(["init", "-q"], at: main)
        try runGit(
            [
                "-c", "user.email=t@t", "-c", "user.name=t",
                "commit", "-q", "--allow-empty", "-m", "init",
            ],
            at: main
        )
        try runGit(["worktree", "add", "-q", worktree.path], at: main)

        let store = SessionsStore(repoPath: worktree.path)

        #expect(
            URL(fileURLWithPath: store.repoPath).resolvingSymlinksInPath().path
                == main.resolvingSymlinksInPath().path
        )
    }
}

@Suite("Workspace Session lifecycle")
@MainActor
struct WorkspaceSessionLifecycleTests {
    @Test("Restoring twelve Sessions prepares only attachments, then switches without preparation")
    func twelveRetainedAttachments() async throws {
        let calls = AttachmentReads()
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { args, _ in
            await calls.record()
            guard args == ["session", "resume", args[2], "--prepare", "--json"] else {
                throw RegistryQueryError("unexpected argv: \(args)")
            }
            return session(id: args[2], state: "detached")
        })
        let ids = (0..<12).map { "session-\($0)" }
        store.reconcile(try records(ids.map { session(id: $0, state: "active") }))
        for id in ids { await store.select(id) }
        let prepared = store.sessions
        #expect(prepared.allSatisfy { $0.state == .prepared })
        #expect(prepared.allSatisfy { $0.launchArguments == ["lf", "session", "resume", $0.id] })
        for _ in 0..<20 {
            for id in ids.reversed() { await store.select(id) }
        }
        #expect(store.sessions == prepared)
        #expect(await calls.count == 12)
    }

    @Test("An ensured running primary attaches when shown")
    func ensuredRunningPrimary() async throws {
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { _, _ in
            session(id: "primary", state: "detached")
        })
        store.reconcile(try records([session(id: "primary", state: "detached")]))
        let ensured = try #require(records([session(id: "primary", state: "detached").replacingOccurrences(
            of: "[\"lf\", \"session\", \"resume\", \"primary\"]", with: "[]")]).first)
        store.adoptPreparation(ensured)
        await store.select(ensured.id)
        #expect(store.sessions.first?.state == .prepared)
        #expect(store.sessions.first?.launchArguments == ["lf", "session", "resume", "primary"])
    }

    @Test("Showing a pane opens it whatever its client is doing", arguments: ["detached", "active", "closed"])
    func showingOpensEveryState(state: String) async throws {
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { args, _ in
            #expect(args == ["session", "resume", "shown", "--prepare", "--json"])
            return session(id: "shown", state: state)
        })
        store.reconcile(try records([session(id: "shown", state: "closed")]))
        await store.select("shown")
        #expect(item(store, "shown")?.state == .prepared)
    }

    @Test("Unavailable preparation preserves the pane identity")
    func attachmentFailureIsStable() async throws {
        let calls = AttachmentReads()
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { args, _ in
            await calls.record()
            throw RegistryQueryError("Execution owner is unavailable")
        })
        let record = try #require(records([session(id: "retained", state: "closed")]).first)
        store.reconcile([record])
        let incarnation = try #require(store.sessions.first?.launchId)
        await store.select(record.id)
        store.reconcile([record])
        #expect(store.sessions.first?.id == record.id)
        #expect(store.sessions.first?.launchId == incarnation)
        #expect(store.sessions.first?.surface == nil)
        #expect(store.sessions.first?.error == "Execution owner is unavailable")
        #expect(await calls.count == 1)
    }

    @Test("Opening again after a transient failure opens the conversation")
    func reopenAfterFailure() async throws {
        let reads = AttachmentReads()
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { args, _ in
            await reads.record()
            #expect(args == ["session", "resume", "retry", "--prepare", "--json"])
            if await reads.count == 1 { throw RegistryQueryError("Temporary transport failure") }
            return session(id: "retry", state: "active")
        })
        let closed = try #require(records([session(id: "retry", state: "closed")]).first)
        store.reconcile([closed])
        await store.select(closed.id)
        store.reconcile([closed])
        let failed = try #require(store.sessions.first)
        #expect(failed.surface == nil)
        let view = recoveryPane(store)._sessionPlaceholder(failed)
        let button = try view.inspect().find(viewWithAccessibilityIdentifier: "session-open-here-retry").button()
        #expect(try button.labelView().text().string() == closed.action?.label)
        try button.tap()
        let deadline = ContinuousClock.now + .seconds(3)
        while store.sessions.first?.state != .prepared, ContinuousClock.now < deadline { await Task.yield() }
        #expect(store.sessions.first?.state == .prepared)
    }

    @Test("A consumed first launch offers Try again from the pane")
    func consumedLaunchRecoversFromThePane() async throws {
        let reads = RecordedArguments()
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { args, _ in
            await reads.record(args)
            return session(id: "consumed", state: "closed")
        })
        let consumed = try consumedLaunchSession()
        store.reconcile([consumed])
        store.reconcile([consumed])
        let pending = try #require(store.sessions.first)
        #expect(pending.state == .pending)
        #expect(await reads.all.isEmpty)

        let view = recoveryPane(store)._sessionPlaceholder(pending)
        let button = try view.inspect().find(viewWithAccessibilityIdentifier: "session-open-here-consumed").button()
        #expect(try button.labelView().text().string() == "Try again")
        try button.tap()
        let deadline = ContinuousClock.now + .seconds(3)
        while store.sessions.first?.state != .prepared, ContinuousClock.now < deadline { await Task.yield() }
        #expect(store.sessions.first?.state == .prepared)
        #expect(await reads.all == [["session", "resume", "consumed", "--prepare", "--json"]])
    }

    @Test("Eviction fences a preparation that returns after release")
    func releaseFencesLatePreparation() async throws {
        let started = AsyncStream<Void>.makeStream(), response = AsyncStream<Void>.makeStream()
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { _, _ in
            started.continuation.yield(())
            for await _ in response.stream { break }
            return session(id: "retained", state: "closed")
        })
        store.reconcile(try records([session(id: "retained", state: "closed")]))
        let selection = Task { await store.select("retained") }
        for await _ in started.stream { break }
        store.release("retained")
        response.continuation.finish()
        await selection.value
        #expect(store.sessions.first?.state == .pending)
        #expect(store.sessions.first?.surface == nil)
    }

    @Test("Automatic preparations start selected first, stay bounded and honor Close")
    func boundedSelectedFirstPreparation() async throws {
        let reads = RevealReads()
        let response = AsyncStream<Void>.makeStream()
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { args, _ in
            let id = args[2]
            await reads.record(id)
            for await _ in response.stream { break }
            return session(id: id, state: "closed")
        })
        store.reconcile(try records(["a", "b", "c", "unseen"].map { session(id: $0, state: "closed") }))
        let registry = SessionsWorkspaceRegistry(localMachineId: fixtureMachineId)
        var workspaces: [SessionsWorkspace] = []
        for id in ["a", "b", "c"] {
            let workspace = registry.open(WorkWorkspaceKey(repository: "repo", kind: .task, id: id), title: id, location: nil)
            workspace.multiplexer.load(sessionId: id)
            workspaces.append(workspace)
        }
        registry.openWarmConversations(store: store)
        let deadline = ContinuousClock.now + .seconds(3)
        while await reads.ids.count < 2, ContinuousClock.now < deadline { await Task.yield() }
        // Slots go out in this order; arrival at the query crosses actors and may swap.
        #expect(registry.warmWork.map(\.id) == ["c", "b", "a"])
        #expect(await reads.ids.sorted() == ["b", "c"])
        #expect(registry.preparationCount == 2)
        workspaces[0].closePane(workspaces[0].multiplexer.sessionPane!.id)
        workspaces[2].closePane(workspaces[2].multiplexer.sessionPane!.id)
        response.continuation.finish()
        while registry.preparationCount > 0, ContinuousClock.now < deadline { await Task.yield() }
        #expect(item(store, "b")?.state == .prepared)
        #expect(item(store, "c")?.surface == nil)
        #expect(item(store, "a")?.surface == nil)
        #expect(item(store, "unseen")?.surface == nil)
        #expect(await reads.ids.sorted() == ["b", "c"])
    }

    @Test("Cancellation after preparation cannot create a late viewer")
    func canceledAttachment() async throws {
        let started = AsyncStream<Void>.makeStream(), release = AsyncStream<Void>.makeStream()
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { _, _ in
            started.continuation.yield(())
            for await _ in release.stream { break }
            return session(id: "retained", state: "detached")
        })
        store.reconcile(try records([session(id: "retained", state: "closed")]))
        let selection = Task { await store.select("retained") }
        for await _ in started.stream { break }
        selection.cancel()
        release.continuation.finish()
        await selection.value
        started.continuation.finish()
        #expect(store.sessions.first?.surface == nil)
        #expect(store.sessions.first?.state == .pending)
    }
}

private actor RevealReads {
    private(set) var ids: [String] = []
    func record(_ id: String) { ids.append(id) }
}

private actor AttachmentReads {
    private(set) var count = 0
    func record() { count += 1 }
}

private actor RecordedArguments {
    private(set) var all: [[String]] = []
    func record(_ args: [String]) { all.append(args) }
}

@MainActor
private func recoveryPane(_ store: SessionsStore) -> SessionPaneView {
    let workspace = SessionsWorkspace(key: .init(repository: "repo", kind: .task, id: "task"),
        title: "Task", location: nil, localMachineId: fixtureMachineId)
    let model = WorkModel(query: store.query, repoPath: store.repoPath)
    return SessionPaneView(pane: workspace.multiplexer.focusedPane, isFocused: true,
        sessions: store, workspace: workspace, model: model, onFocus: { _, _ in })
}

private func consumedLaunchSession() throws -> SessionRecord {
    var wire = try #require(JSONSerialization.jsonObject(with: Data(session(id: "consumed", state: "closed").utf8)) as? [String: Any])
    wire["action"] = ["label": "Try again", "help": "The previous launch ended before the provider saved any history; this repeats its request"]
    return try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: wire))
}

@MainActor
private func item(_ store: SessionsStore, _ id: String) -> SessionItem? {
    store.sessions.first { $0.id == id }
}

private func records(_ entries: [String]) throws -> [SessionRecord] {
    try JSONDecoder().decode(
        [SessionRecord].self,
        from: Data("[\(entries.joined(separator: ","))]".utf8)
    )
}

private func session(id: String, state: String, primary: Bool = false) -> String {
    // A conversation available for a person has no client; `available` is its attention.
    let wire = state == "available" ? "unknown" : state
    return """
    {
      "id": "\(id)", "run_id": "\(id)", "interactive": true,
      "work": { "kind": "task", "id": "task-\(id)" },
      "title": "Design the control surface",
      "detail": "review-design",
      "cwd": "/tmp/repo.\(id)",
      "workspace": {"machine_id": "\(fixtureMachineId)", "worktree": "/tmp/repo.\(id)", "task_id": null, "unavailable": null},
      "state": "\(wire)", "availability": \(state == "available" ? "\"available\"" : "null"),
      "work_path": "product / Desktop / LOO-291",
      "action": \(sessionActionFixtureJSON(state: wire)),
      "blocked": false, "title_source": "generated", "ended": null, "start": null, "outlook": null, "status_line": null, "task_primary": \(primary), "graph_membership": {"kind": "independent"}, "task_ids": ["task-\(id)"], "agent_process_id": "44444444-4444-4444-8444-444444444444", "turn":null,"raw":false,"terminal_ids": [],
      "open_argv": ["lf", "session", "resume", "\(id)"]
    }
    """
}

private func runGit(_ args: [String], at directory: URL) throws {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
    process.arguments = ["git", "-C", directory.path] + args
    process.standardOutput = Pipe()
    process.standardError = Pipe()
    try process.run()
    process.waitUntilExit()
    try #require(process.terminationStatus == 0, "git \(args.joined(separator: " "))")
}

#endif
