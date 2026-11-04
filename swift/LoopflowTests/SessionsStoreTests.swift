#if os(macOS)
import Foundation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Sessions store")
@MainActor
struct SessionsStoreTests {
    @Test("Provider switching keeps the Session and prepares an explicit provider launch", arguments: ["claude", "codex", "opencode", "pi"])
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
        #expect(prepared.record.agentProcessId == "new-agent")
        #expect(prepared.launchArguments == launch)
        #expect(SessionProviderReading(record: prepared.record, terminal: "current-terminal")?.agentProcessId == "new-agent")
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

    @Test("Reset restarts the same Session with a fresh launch")
    func resetRestartsSameSession() async throws {
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { args, _ in
            guard args == ["session", "new", "main", "--prepare", "--json"] else {
                throw RegistryQueryError("unexpected argv: \(args)")
            }
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

    @Test("Explicit Start opens only the requested terminal")
    func startsOnlyTheSelectedSession() async throws {
        let store = SessionsStore(
            repoPath: "/tmp/repo",
            query: RegistryQuery { args, _ in
                #expect(args == ["session", "resume", "--start-if-free", "second", "--prepare", "--json"])
                return session(id: "second", state: "available")
            }
        )
        store.reconcile(try records([
            session(id: "first", state: "available"),
            session(id: "second", state: "available"),
        ]))

        await store.start("second")

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

        await store.start(prepared.id)

        #expect(item(store, prepared.id)?.state == .prepared)
        #expect(item(store, prepared.id)?.surface?.openArgv == prepared.openArgv)
        #expect(item(store, prepared.id)?.error == nil)
    }

    @Test("A Session opened elsewhere during selection is not prepared locally")
    func selectionHonorsFreshClientOwnership() async throws {
        let store = SessionsStore(
            repoPath: "/tmp/repo",
            query: RegistryQuery { _, _ in session(id: "native", state: "active") }
        )
        store.reconcile(try records([session(id: "native", state: "available")]))

        await store.start("native")

        #expect(item(store, "native")?.surface == nil)
        #expect(item(store, "native")?.state == .elsewhere)
        #expect(item(store, "native")?.error == nil)
    }

    @Test("Selecting an active Session leaves its other terminal running until Move here")
    func interactiveSelectionRequiresExplicitMove() async throws {
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

        await store.start("native")
        #expect(item(store, "native")?.surface == nil)
        #expect(item(store, "native")?.state == .elsewhere)

        await store.moveHere("native")

        #expect(item(store, "native")?.state == .prepared)
        #expect(item(store, "native")?.surface?.openArgv == ["lf", "session", "resume", "native"])
    }

    @Test("Explicit reopen revalidates a cached observation failure", arguments: ["available", "refused", "unavailable"])
    func reopenRevalidatesCachedFailure(result: String) async throws {
        let reason = "Session client observation unavailable"
        let record = session(id: "native", state: "closed")
        let blocked = record.replacingOccurrences(of: "\"unavailable_reason\":null", with: "\"unavailable_reason\":\"\(reason)\"")
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { _, _ in
            if result == "refused" { throw RegistryQueryError(reason) }
            return result == "unavailable" ? blocked : record
        })
        store.reconcile(try records([blocked]))
        #expect(item(store, "native")?.record.action(.open)?.unavailableReason == reason)

        await store.start("native")

        if result == "available" {
            #expect(item(store, "native")?.state == .prepared)
            #expect(item(store, "native")?.surface?.openArgv == ["lf", "session", "resume", "native"])
        } else {
            #expect(item(store, "native")?.surface == nil)
            #expect(item(store, "native")?.error?.contains(reason) == true)
        }
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

        if replacing {
            await store.moveHere("native")
        } else {
            await store.start("native")
        }
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

    @Test("An externally killed terminal reclassifies as active elsewhere")
    func externalKillReclassifies() async throws {
        let store = SessionsStore(
            repoPath: "/tmp/repo",
            query: RegistryQuery { _, _ in
                session(id: "native", state: "active")
            }
        )
        store.reconcile(try records([
            session(id: "native", state: "active"),
        ]))
        await store.moveHere("native")
        store.recordPaneLive("native")
        #expect(item(store, "native")?.state == .live)

        store.noteSurfaceClosed(.shell("native", machineId: fixtureMachineId))
        #expect(item(store, "native")?.state == .live)
        store.noteSurfaceClosed(.session("native", machineId: "peer"))
        #expect(item(store, "native")?.state == .live)

        store.noteSurfaceClosed(.session("native", machineId: fixtureMachineId))

        #expect(item(store, "native")?.state == .elsewhere)
        #expect(item(store, "native")?.surface == nil)
    }

    @Test("Ended Session output survives a fresh inventory without relaunch")
    func endedOutputSurvivesPolling() async throws {
        let record = session(id: "native", state: "closed")
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { _, _ in record })
        store.reconcile(try records([record]))
        await store.start("native")
        store.recordPaneLive("native")
        store.noteSurfaceClosed(.session("native", machineId: fixtureMachineId),
            output: "Error: Previous AgentProcess has no observed exit")
        store.reconcile(try records([record]))
        #expect(item(store, "native")?.state == .ended("Error: Previous AgentProcess has no observed exit"))
        #expect(item(store, "native")?.surface == nil)
        await store.start("native")
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

        await store.start("native")
        #expect(item(store, "native")?.error != nil)

        store.reconcile(try records([
            session(id: "native", state: "closed"),
        ]))
        #expect(item(store, "native")?.error != nil)

        store.reconcile(try records([
            session(id: "native", state: "active"),
        ]))
        #expect(item(store, "native")?.state == .elsewhere)
    }


    @Test("Retained review feedback stays visible")
    func retainedFeedbackDoesNotDisappear() throws {
        let store = SessionsStore(repoPath: "/tmp/repo")
        store.reconcile(try records([session(id: "review", state: "available")]))

        #expect(store.sessions.map(\.id) == ["review"])
        #expect(store.sessions.first?.record.availability == .available)
        #expect(store.sessions.first?.record.readySummary == "Ready for review")
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
            guard args == ["session", "resume", "--reveal-existing", args[3], "--prepare", "--json"] else {
                throw RegistryQueryError("unexpected argv: \(args)")
            }
            return session(id: args[3], state: "detached")
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

    @Test("Showing a pane resumes saved history and leaves external input alone",
          arguments: [("detached", SessionItem.State.prepared), ("active", .elsewhere), ("closed", .prepared)])
    func showingDecidesFromState(state: String, expected: SessionItem.State) async throws {
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { args, _ in
            #expect(args.contains("--reveal-existing"))
            return session(id: "shown", state: state)
        })
        store.reconcile(try records([session(id: "shown", state: "closed")]))
        await store.select("shown")
        #expect(item(store, "shown")?.state == expected)
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

    @Test("Missing history remains an explicit Start through refresh and rendering")
    func missingHistoryRecovery() async throws {
        let missing = try missingHistorySession()
        let wire = String(decoding: try JSONEncoder().encode(missing), as: UTF8.self)
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { args, _ in
            if args.contains("--reveal-existing") { return wire }
            #expect(args.contains("--start-if-free"))
            var started = missing
            started.openArgv.append("--start-if-free")
            return String(decoding: try JSONEncoder().encode(started), as: UTF8.self)
        })
        store.reconcile([missing])
        await store.select(missing.id)
        store.reconcile([missing])
        let before = try #require(store.sessions.first)
        #expect(before.state == .unavailable(try #require(missing.action(.reveal)?.unavailableReason)))
        #expect(before.surface == nil)
        #expect(before.lastScreen == nil)
        let view = recoveryPane(store)._sessionPlaceholder(before)
        #expect(throws: (any Error).self) { try view.inspect().find(text: "No longer open here") }
        let button = try view.inspect().find(viewWithAccessibilityIdentifier: "session-open-here-\(missing.id)").button()
        #expect(try button.labelView().text().string() == missing.action(.open)?.label)
        try button.tap()
        let deadline = ContinuousClock.now + .seconds(3)
        while store.sessions.first?.surface == nil, ContinuousClock.now < deadline { await Task.yield() }
        #expect(store.sessions.first?.state == .prepared)
        #expect(store.sessions.first?.launchArguments == ["lf", "session", "resume", missing.id, "--start-if-free"])
    }

    @Test("Fresh ownership replaces unavailable opening with Take back")
    func missingHistoryThenHolder() async throws {
        let missing = try missingHistorySession()
        let store = SessionsStore(repoPath: "/tmp/repo")
        store.reconcile([missing])
        store.adoptPreparation(missing, intent: .reveal)
        let active = try #require(records([session(id: missing.id, state: "active")]).first)
        store.reconcile([active])
        let current = try #require(store.sessions.first)
        #expect(current.state == .elsewhere)
        #expect(current.surface == nil)
        let view = recoveryPane(store)._sessionPlaceholder(current)
        let button = try view.inspect().find(viewWithAccessibilityIdentifier: "session-move-here-\(missing.id)").button()
        #expect(try button.labelView().find(text: try #require(active.action(.moveHere)?.label)).string() == active.action(.moveHere)?.label)
        #expect(throws: (any Error).self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "session-open-here-\(missing.id)")
        }
    }

    @Test("A prepared Start stays non-replacing through refresh; only Take back transfers input")
    func startThenCompetingHolder() async throws {
        let missing = try missingHistorySession()
        let active = try #require(records([session(id: missing.id, state: "active")]).first)
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { args, _ in
            var record = args.contains("--start-if-free") ? missing : active
            if args.contains("--start-if-free") { record.openArgv.append("--start-if-free") }
            return String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        })
        store.reconcile([missing])
        await store.start(missing.id)
        #expect(store.sessions.first?.state == .prepared)
        store.reconcile([active])
        #expect(store.sessions.first?.launchArguments.contains("--start-if-free") == true)
        store.recordPaneFailure(missing.id, reason: "Input was acquired elsewhere")
        store.reconcile([active])
        #expect(store.sessions.first?.state == .elsewhere)
        await store.moveHere(missing.id)
        #expect(store.sessions.first?.state == .prepared)
        #expect(store.sessions.first?.launchArguments == active.openArgv)
        #expect(store.sessions.count == 1)
    }

    @Test("Fresh saved history clears an unavailable opening without starting it")
    func missingHistoryThenSavedHistory() throws {
        let missing = try missingHistorySession()
        let store = SessionsStore(repoPath: "/tmp/repo")
        store.reconcile([missing])
        store.adoptPreparation(missing, intent: .reveal)
        store.reconcile(try records([session(id: missing.id, state: "closed")]))
        #expect(store.sessions.first?.state == .pending)
        #expect(store.sessions.first?.surface == nil)
    }

    @Test("Retry after a transient failure only reveals and retains a newly observed holder")
    func retryRecoveryNeverReplaces() async throws {
        let reads = AttachmentReads()
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { args, _ in
            await reads.record()
            #expect(args.contains("--reveal-existing"))
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
        #expect(try button.labelView().text().string() == "Retry")
        try button.tap()
        let deadline = ContinuousClock.now + .seconds(3)
        while store.sessions.first?.state != .elsewhere, ContinuousClock.now < deadline { await Task.yield() }
        #expect(store.sessions.first?.state == .elsewhere)
        #expect(store.sessions.first?.surface == nil)
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
            let id = args[3]
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
        registry.revealConversations(store: store)
        let deadline = ContinuousClock.now + .seconds(3)
        while await reads.ids.count < 2, ContinuousClock.now < deadline { await Task.yield() }
        #expect(await reads.ids == ["c", "b"])
        #expect(registry.preparationCount == 2)
        workspaces[0].closePane(workspaces[0].multiplexer.sessionPane!.id)
        workspaces[2].closePane(workspaces[2].multiplexer.sessionPane!.id)
        response.continuation.finish()
        while registry.preparationCount > 0, ContinuousClock.now < deadline { await Task.yield() }
        #expect(item(store, "b")?.state == .prepared)
        #expect(item(store, "c")?.surface == nil)
        #expect(item(store, "a")?.surface == nil)
        #expect(item(store, "unseen")?.surface == nil)
        #expect(await reads.ids == ["c", "b"])
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

@MainActor
private func recoveryPane(_ store: SessionsStore) -> SessionPaneView {
    let workspace = SessionsWorkspace(key: .init(repository: "repo", kind: .task, id: "task"),
        title: "Task", location: nil, localMachineId: fixtureMachineId)
    let model = WorkModel(query: store.query, repoPath: store.repoPath)
    return SessionPaneView(pane: workspace.multiplexer.focusedPane, isFocused: true,
        sessions: store, workspace: workspace, model: model, onFocus: { _, _ in })
}

private func missingHistorySession() throws -> SessionRecord {
    var wire = try #require(JSONSerialization.jsonObject(with: Data(session(id: "missing", state: "closed").utf8)) as? [String: Any])
    wire["actions"] = [
        ["kind": "open", "label": "Start conversation", "help": "Start explicitly", "unavailable_reason": NSNull()],
        ["kind": "reveal", "label": "Show conversation", "help": "Existing history", "unavailable_reason": "No saved provider history; start this conversation explicitly"],
    ]
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
      "ready_summary": \(state == "available" ? "\"Ready for review\"" : "null"),
      "work_path": "product / Desktop / LOO-291",
      "actions": \(sessionActionFixtureJSON(state: wire)),
      "title_source": "generated", "task_primary": \(primary), "graph_membership": {"kind": "independent"}, "task_ids": ["task-\(id)"], "agent_process_id": "44444444-4444-4444-8444-444444444444", "terminal_ids": [],
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
