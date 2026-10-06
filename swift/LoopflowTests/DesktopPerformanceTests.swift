#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
import ViewInspector
import Vision
#if canImport(GhosttyKit)
import GhosttyKit
#endif
@testable import Loopflow
@testable import LoopflowMac

/// Opt-in native benchmark. The endpoint is captured native pixels + owned PTY input,
/// not compositor presentation. Run through scripts/desktop_performance.py.
@Suite("Desktop experience measurements", .requiresDisplay, .serialized)
@MainActor
struct DesktopPerformanceTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["LF_DESKTOP_PERF_OUTPUT"] != nil))
    func measureExperiences() async throws {
        let environment = ProcessInfo.processInfo.environment
        let output = URL(fileURLWithPath: try #require(environment["LF_DESKTOP_PERF_OUTPUT"]))
        let samples = try #require(Int(environment["LF_DESKTOP_PERF_SAMPLES"] ?? "20"))
        let journal = try PerformanceJournal(url: output)
        let soakSeconds = Double(environment["LF_DESKTOP_PERF_SOAK_SECONDS"] ?? "0") ?? 0
        let scenarios = ["full", "fold", "expand", "compact", "sessions", "filter", "scroll_refresh",
                         "monitor_active", "monitor_empty", "session_return", "combined_zoom", "combined_restore", "task_details", "task_flow", "task_history", "file_edit_refresh", "new_pty"]
        try journal.write(["event": "plan", "population_version": "desktop-v3",
                           "populations": ["small": 8, "large": 256], "samples": samples,
                           "scenarios": scenarios, "endpoint": "native_capture_ocr_and_pty_reply",
                           "poll_interval_ms": 5, "frame_hitches": NSNull(), "soak_seconds": soakSeconds,
                           "scroll_refresh": ["window_height": 300, "destination": "end",
                                              "refresh_release": "after_captured_scroll", "updated_title_prefix": "Updated"],
                           "gaps": ["Compositor presentation and frame hitches are not measured.",
                                    "Planning transport is synthetic; native reopening uses the real CLI with an owned provider stub.",
                                    "Actions use SwiftUI controls; OS event delivery latency is excluded.",
                                    "Forced bitmap capture and OCR are intrusive observer costs, recorded separately."]])
#if canImport(GhosttyKit)
        _ = NSApplication.shared
        NSApp.setActivationPolicy(.accessory)
        NSApp.finishLaunching()
        guard NSScreen.main != nil else {
            try journal.write(["event": "setup", "outcome": "unavailable", "reason": "No native screen"])
            throw PerformanceFailure("unavailable", "No native screen")
        }
        GhosttyManager.shared.initialize()
        guard GhosttyManager.shared.state == .ready else {
            try journal.write(["event": "setup", "outcome": "unavailable", "reason": "Ghostty initialization failed"])
            throw PerformanceFailure("unavailable", "Ghostty initialization failed")
        }
        for (population, taskCount) in [("small", 8), ("large", 256)] {
            do {
                try await measure(population: population, taskCount: taskCount, samples: samples, soakSeconds: population == "large" ? soakSeconds : 0, journal: journal)
            } catch {
                try journal.write(["event": "setup_or_journey", "population": population,
                                   "outcome": (error as? PerformanceFailure)?.outcome ?? "failed",
                                   "reason": String(describing: error)])
                throw error
            }
        }
#else
        try journal.write(["event": "setup", "outcome": "unavailable", "reason": "GhosttyKit is absent"])
        throw PerformanceFailure("unavailable", "GhosttyKit is absent")
#endif
    }

#if canImport(GhosttyKit)
    private func measure(population: String, taskCount: Int, samples: Int, soakSeconds: Double, journal: PerformanceJournal) async throws {
        let checkout = FileManager.default.temporaryDirectory.appendingPathComponent("desktop-perf-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: checkout, withIntermediateDirectories: true)
        try Data("Original notes".utf8).write(to: checkout.appendingPathComponent("notes.txt"))
        defer { try? FileManager.default.removeItem(at: checkout) }
        let (query, planning) = try populationQuery(taskCount: taskCount, checkout: checkout.path)
        let detailWindow = PerformanceWindow(contentRect: CGRect(x: 0, y: 0, width: 1100, height: 800),
                                            styleMask: [.titled], backing: .buffered, defer: false)
        detailWindow.isReleasedWhenClosed = false
        defer { detailWindow.contentView = nil; detailWindow.close() }
        let model = PodiumModel(query: query, repoPath: checkout.path)
        await model.refresh()
        try #require(model.workspace.waves.first?.tasks.count == taskCount)
        try #require(model.sessions.value?.count == taskCount / 2)
        let registry = SessionsWorkspaceRegistry(localHomeId: fixtureHomeId)
        let workspace = registry.workspace(for: fixtureWorkspace(checkout.path))
        let files = workspace.files(taskId: "perf-work-0", issue: "perf-work-0", cwd: checkout.path, query: query)
        files.autosave = false
        let multiplexer = workspace.multiplexer
        multiplexer.load(sessionId: "perf-session-0")
        let sessionPane = multiplexer.focusedPaneId
        multiplexer.newShell()
        let companionPane = multiplexer.focusedPaneId
        multiplexer.load(sessionId: "perf-session-2")
        let otherPane = multiplexer.focusedPaneId
        let identities: [TerminalIdentity] = [.session("perf-session-0"), .shell(companionPane), .session("perf-session-2")]
        let terminals = identities.map { registry.surfaces.view(for: $0) }
        defer { for identity in identities { registry.surfaces.release(identity) } }
        for terminal in terminals {
            terminal.frame = CGRect(x: 0, y: 0, width: 450, height: 350)
            terminal.workingDirectory = NSTemporaryDirectory()
            terminal.command = buildGhosttyShellCommand(argv: ["/bin/cat"], env: [:])
            terminal.createSurface(manager: GhosttyManager.shared)
        }
        let surfaces = try terminals.map { try #require($0.surface) }
        multiplexer.setFocusedPane(sessionPane)
        model.select(.task(id: "perf-task-0"))
        model.navigation.content = .terminals
        let view = SessionsView(model: model, repoPath: checkout.path, workspaces: registry, query: query)
        let window = PerformanceWindow(contentRect: CGRect(x: 0, y: 0, width: 1400, height: 800),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: view)
        window.orderFront(nil)
        defer { window.contentView = nil; window.close() }
        try await wait(window, render: true) { hasRendered(window, "workspace-task-perf-task-0") }
        let navigator = try view.inspect().find(WorkspaceNavigator.self).actualView()
        let picker = try navigator.inspect().find(ViewType.Picker.self)
        let search = try navigator.inspect().find(ViewType.TextField.self)
        let records = try #require(model.sessions.value)
        let first = try #require(records.first { $0.id == "perf-session-0" })
        let other = try #require(records.first { $0.id == "perf-session-2" })
        let openTask = try #require(navigator.onOpenTask)
        let selected = try #require(model.task(id: "perf-task-0"))

        for attempt in 0..<samples {
            // Reset outside the measured interval; every attempt starts from the
            // same visible population and retained surfaces, including the first.
            try search.setInput("")
            try picker.select(value: WorkspacePresentation.compact)
            try await wait(window, render: true) { hasRendered(window, "workspace-task-perf-task-0") }
            try await sample("full", population, attempt, journal, window, action: {
                try picker.select(value: WorkspacePresentation.full)
            }, ready: { hasRendered(window, "workspace-wave-wave-1") && hasRendered(window, "workspace-task-perf-task-0") })
            let disclosure = try navigator.inspect().find(viewWithAccessibilityIdentifier: "workspace-disclose-wave-wave-1").button()
            try await sample("fold", population, attempt, journal, window, action: {
                try disclosure.tap()
            }, ready: { hasRendered(window, "workspace-wave-wave-1") && !hasRendered(window, "workspace-task-perf-task-0") })
            try await sample("expand", population, attempt, journal, window, action: {
                try disclosure.tap()
            }, ready: { hasRendered(window, "workspace-task-perf-task-0")  })
            try await sample("compact", population, attempt, journal, window, action: {
                try picker.select(value: WorkspacePresentation.compact)
            }, ready: { hasRendered(window, "workspace-task-perf-task-0") && !hasRendered(window, "workspace-wave-wave-1") })
            try await sample("sessions", population, attempt, journal, window, action: {
                try picker.select(value: WorkspacePresentation.sessions)
            }, ready: { hasRendered(window, "session-row-perf-session-0") && !hasRendered(window, "workspace-task-perf-task-0") })
            try await sample("filter", population, attempt, journal, window, action: {
                try search.setInput("Conversation 002")
            }, ready: { hasRendered(window, "session-row-perf-session-2") && !hasRendered(window, "session-row-perf-session-0") })
            try #require(model.selection == .task(id: "perf-task-0"))
            try search.setInput("")
            try picker.select(value: WorkspacePresentation.compact)
            // Keep the same population; a shorter viewport makes both sizes
            // scrollable. Resize and start the held read outside the interval.
            window.setContentSize(NSSize(width: 1400, height: 300))
            try await wait(window, render: true) { hasRendered(window, "workspace-task-perf-task-0") }
            let scroll = try #require(window.contentView.flatMap { scrollView(in: $0) })
            let document = try #require(scroll.documentView)
            let destination = document.bounds.height - scroll.contentView.bounds.height
            try #require(destination > 0)
            planning.holdNextRead = true
            let refresh = Task { await model.refresh() }
            defer { planning.release() }
            try await wait(window) { planning.pending != nil }
            var scrolledOffset: CGFloat = 0
            var labelsBeforeRefresh: [String] = []
            var initialVerificationMS: Double = 0
            try await sample("scroll_refresh", population, attempt, journal, window, action: {
                scroll.contentView.scroll(to: CGPoint(x: 0, y: destination))
                scroll.reflectScrolledClipView(scroll.contentView)
                // Lazy row measurement can correct the estimated content height.
                // Capture the destination before judging retention across refresh.
                try await wait(window, render: true) {
                    hasRendered(window, "workspace-task-perf-task-\(taskCount - 1)")
                }
                scrolledOffset = scroll.contentView.bounds.minY
                labelsBeforeRefresh = window.outlineText
                initialVerificationMS = milliseconds(window.observedAt)
                try #require(scrolledOffset > 0 && model.isRefreshing)
                planning.release()
            }, ready: {
                !model.isRefreshing && abs(scroll.contentView.bounds.minY - scrolledOffset) < 1
                    && model.selection == .task(id: "perf-task-0")
                    && model.sessions.value == records
                    && model.workspace.waves.first?.tasks.last?.task.task.id == "perf-task-\(taskCount - 1)"
                    && window.outlineText.contains { $0.contains(String(format: "Updated %03d", taskCount - 1)) }
                    && !window.outlineText.contains { $0.contains("Updated 000") }
            }, observation: {
                ["offset_after_scroll": scrolledOffset, "offset_after_refresh": scroll.contentView.bounds.minY,
                 "refresh_complete": !model.isRefreshing, "visible_labels": window.outlineText,
                 "visible_labels_before_refresh": labelsBeforeRefresh,
                 "initial_capture_verification_ms": initialVerificationMS,
                 "expected_task_id": "perf-task-\(taskCount - 1)"]
            })
            await refresh.value
            await model.refresh()
            window.setContentSize(NSSize(width: 1400, height: 800))
            scroll.contentView.scroll(to: .zero)
            scroll.reflectScrolledClipView(scroll.contentView)
            try await wait(window, render: true) { hasRendered(window, "workspace-task-perf-task-0") }
            navigator.onOpenSession(first)
            try await wait(window) { window.firstResponder === terminals[0] }
            let draft = "draft-\(attempt)"
            try send(window, draft)
            try await wait(window) { terminalText(surfaces[0]).contains(draft) }
            navigator.onOpenSession(other)
            try await wait(window) { window.firstResponder === terminals[2] && multiplexer.focusedPaneId == otherPane }
            openTask(.task(id: "perf-task-0"))
            try await wait(window) { window.firstResponder === terminals[0] }
            let monitorButton = try view.inspect().find(ViewType.Button.self, where: {
                try $0.accessibilityIdentifier() == "task-show-monitor-perf-task-0"
            })
            try await sample("monitor_active", population, attempt, journal, window, action: {
                // This Task already has a retained Session choice; the explicit
                // Monitor control reveals observation alongside it.
                try monitorButton.tap()
            }, ready: {
                !model.isRefreshingActiveSessions && hasRendered(window, "monitor-session-perf-session-0")
                    && !hasRendered(window, "monitor-session-perf-session-2")
                    && !terminals.contains { window.firstResponder === $0 }
                    && multiplexer.focusedPane.content == .monitor(taskId: "perf-task-0")
            })
            let monitorPane = multiplexer.focusedPaneId
            try await sample("monitor_empty", population, attempt, journal, window, action: {
                // A Task without Sessions opens its overview; Monitor is explicit.
                openTask(.task(id: "perf-task-1"))
                try view.inspect().find(ViewType.Button.self, where: {
                    try $0.accessibilityIdentifier() == "task-show-monitor-perf-task-1"
                }).tap()
            }, ready: { !model.isRefreshingActiveSessions && hasRendered(window, "monitor-empty-perf-task-1") })
            try await sample("session_return", population, attempt, journal, window, action: {
                navigator.onOpenSession(first)
            }, ready: {
                guard window.firstResponder === terminals[0], terminals[0].window === window,
                      terminals[0].surface == surfaces[0], terminals[1].surface == surfaces[1],
                      terminals[2].surface == surfaces[2],
                      model.navigation.selectedSessionId == first.id,
                      multiplexer.focusedPane.content == .session(id: first.id) else { return false }
                return window.contentText.contains { $0.contains("Conversation 000") }
            }, input: {
                try send(window, "\n")
                try await wait(window) { terminalText(surfaces[0]).components(separatedBy: draft).count >= 3 }
                // The companion uses its existing surface and process too.
                let reply = "companion-\(attempt)"
                let text = reply + "\n"
                text.withCString { ghostty_surface_text(surfaces[1], $0, UInt(text.utf8.count)) }
                try await wait(window) { terminalText(surfaces[1]).components(separatedBy: reply).count >= 3 }
            })
            multiplexer.updateRatio(between: sessionPane, and: monitorPane, ratio: 0.5)
            try await sample("combined_zoom", population, attempt, journal, window, action: {
                multiplexer.setFocusedPane(monitorPane)
                multiplexer.toggleZoom(monitorPane)
            }, ready: { hasRendered(window, "monitor-session-perf-session-0") && terminals[0].window == nil })
            try await sample("combined_restore", population, attempt, journal, window, action: {
                multiplexer.toggleZoom(monitorPane)
                multiplexer.updateRatio(between: sessionPane, and: monitorPane, ratio: 0.6)
                navigator.onOpenSession(first)
            }, ready: {
                window.firstResponder === terminals[0] && terminals.allSatisfy { $0.window === window }
                    && hasRendered(window, "monitor-session-perf-session-0")
            })
            try await sample("task_details", population, attempt, journal, window, action: {
                model.select(.task(id: "perf-task-0"))
                try pressElement("breadcrumb-task", in: window)
            }, ready: { window.attachedSheet != nil && window.allText.contains { $0.contains("Task 000") } })
            try await sample("task_flow", population, attempt, journal, window, action: {
                try pressElement("flow-node-2", in: window)
            }, ready: {
                model.navigation.flowDrafts[selected.task.id]?.selectedNode?.node == 2
                    && window.allText.contains { $0.contains("realign") }
            })
            model.navigation.expandedHistory.remove(selected.task.id)
            try await sample("task_history", population, attempt, journal, window, action: {
                try pressElement("task-history-toggle", in: window)
                try await wait(window) { model.sessionHistory[selected.task.id].value?.count == taskCount }
                try revealElement("task-history-list", in: window)
            }, ready: {
                model.navigation.expandedHistory.contains(selected.task.id)
                    && model.sessionHistory[selected.task.id].value?.count == taskCount
                    && window.allText.contains { $0.contains("implement") }
            })
            workspace.showsDetails = false
            try await wait(window) { window.attachedSheet == nil }
            try await sample("file_edit_refresh", population, attempt, journal, window, action: {
                try pressElement("workspace-toggle-files", in: window)
                try await wait(window) { files.directories[""]?.entries.first?.path == "notes.txt" }
                try pressElement("notes.txt", in: window, property: "Label")
                try await wait(window) { files.selectedDocument?.snapshot != nil }
                let document = try #require(files.selectedDocument)
                document.editor.insertText("Preserved draft", replacementRange: NSRange(location: 0, length: document.editor.string.utf16.count))
                document.editor.setSelectedRange(NSRange(location: 2, length: 3))
                await files.refresh()
                await files.loadFile()
            }, ready: {
                files.selectedDocument?.text == "Preserved draft"
                    && files.selectedDocument?.editor.selectedRange() == NSRange(location: 2, length: 3)
                    && window.allText.contains { $0.contains("Preserved draft") }
            })
            try pressElement("workspace-toggle-files", in: window)
            detailWindow.orderFront(nil)
            let freshIdentity = TerminalIdentity.shell("benchmark-\(UUID().uuidString)")
            let fresh = registry.surfaces.view(for: freshIdentity)
            defer { registry.surfaces.release(freshIdentity) }
            try await sample("new_pty", population, attempt, journal, detailWindow, action: {
                fresh.frame = CGRect(x: 0, y: 0, width: 1100, height: 800)
                fresh.workingDirectory = NSTemporaryDirectory()
                fresh.command = buildGhosttyShellCommand(argv: ["/bin/cat"], env: [:])
                detailWindow.contentView = fresh
                fresh.createSurface(manager: GhosttyManager.shared)
                let surface = try #require(fresh.surface)
                let text = "New terminal ready\n"
                text.withCString { ghostty_surface_text(surface, $0, UInt(text.utf8.count)) }
                try await wait(detailWindow) {
                    terminalText(surface).components(separatedBy: "New terminal ready").count >= 3
                }
            }, ready: { detailWindow.allText.contains { $0.contains("New terminal ready") } })
            detailWindow.contentView = nil
            detailWindow.orderOut(nil)
            navigator.onOpenSession(first)
            try await wait(window) { window.firstResponder === terminals[0] }
            guard terminals.enumerated().allSatisfy({ $0.element.surface == surfaces[$0.offset] }),
                  multiplexer.layout.pane(for: companionPane)?.content == .shell,
                  multiplexer.layout.pane(for: otherPane)?.content == .session(id: other.id) else {
                throw PerformanceFailure("failed", "Retained terminal identity or companion changed")
            }
        }
        if soakSeconds > 0 {
            // One owner keeps the real refresh cadences running for the whole soak.
            let refresh = Task { await model.keepWorkspaceCurrent() }
            defer { refresh.cancel() }
            let retainedLayout = multiplexer.layout
            let start = ContinuousClock.now
            var round = 0
            try journal.write(["event": "soak_begin", "pid": ProcessInfo.processInfo.processIdentifier,
                               "seconds": soakSeconds, "time": Date().timeIntervalSince1970])
            while start.duration(to: .now) < .seconds(soakSeconds) {
                try journal.write(["event": "soak_phase", "phase": "idle", "round": round,
                                   "time": Date().timeIntervalSince1970, "reads": planning.reads])
                // Yield to AppKit, without forcing bitmap capture during idle.
                let remaining = Double(soakSeconds) - Double(start.duration(to: .now).components.seconds)
                try await Task.sleep(for: .seconds(min(30, max(0, remaining))))
                if start.duration(to: .now) >= .seconds(soakSeconds) { break }
                try journal.write(["event": "soak_phase", "phase": "navigation_typing", "round": round,
                                   "time": Date().timeIntervalSince1970])
                for record in [other, first] {
                    navigator.onOpenSession(record)
                    let index = record.id == first.id ? 0 : 2
                    try await wait(window) { window.firstResponder === terminals[index] }
                    let text = "soak-\(round)-\(index)"
                    try send(window, text + "\n")
                    try await wait(window) { terminalText(surfaces[index]).components(separatedBy: text).count >= 3 }
                }
                multiplexer.toggleZoom(sessionPane)
                multiplexer.toggleZoom(sessionPane)
                try #require(terminals.enumerated().allSatisfy { $0.element.surface == surfaces[$0.offset] })
                try #require(files.selectedDocument?.text == "Preserved draft")
                try #require(multiplexer.layout == retainedLayout)
                try #require(model.sessions.value == records)
                try journal.write(["event": "soak_round", "round": round, "reads": planning.reads,
                                   "time": Date().timeIntervalSince1970, "windows": NSApp.windows.filter(\.isVisible).count,
                                   "focused_session": model.navigation.selectedSessionId ?? "none", "preserved": true])
                round += 1
            }
            refresh.cancel()
            await refresh.value
            try journal.write(["event": "soak_end", "elapsed_seconds": Double(start.duration(to: .now).components.seconds),
                               "rounds": round, "preserved": true, "reads": planning.reads, "time": Date().timeIntervalSince1970])
            // Let the recorder finish its fixed-duration attachment before this
            // test process exits. The tail is outside the measured soak.
            try await waitForResourceRecorder()
        }
        await model.stopActiveSessions()
    }

    private func measureOwnedSoak(view: SessionsView, window: PerformanceWindow,
                                  fixture: DesktopNativeSessionFixture, query: RegistryQuery,
                                  samples: Int, seconds: Double, journal: PerformanceJournal) async throws {
        let model = view.model
        let registry = view.workspaces
        let record = try #require(try await fixture.records().first)
        try #require(record.taskIds.contains(fixture.taskId))
        let identity = TerminalIdentity.session(fixture.sessionId)
        let workspace = registry.workspace(for: try #require(record.workspace).identity)
        let store = workspace.sessionStore(repoPath: fixture.repo, query: query)
        let multiplexer = workspace.multiplexer
        multiplexer.load(sessionId: fixture.sessionId)
        let sessionPane = multiplexer.focusedPaneId
        multiplexer.newShell()
        let companion = multiplexer.focusedPaneId
        let companionIdentity = TerminalIdentity.shell(companion)
        let companionView = registry.surfaces.view(for: companionIdentity)
        companionView.workingDirectory = fixture.checkout
        companionView.command = buildGhosttyShellCommand(argv: ["/bin/cat"], env: [:])
        companionView.createSurface(manager: GhosttyManager.shared)
        let companionSurface = try #require(companionView.surface)
        multiplexer.setFocusedPane(sessionPane)
        let layout = multiplexer.layout
        defer {
            registry.surfaces.release(identity)
            registry.surfaces.release(companionIdentity)
        }
        let files = workspace.files(taskId: fixture.taskId, issue: fixture.issue, cwd: fixture.checkout, query: query)
        files.autosave = false
        files.selection = "notes.txt"
        await files.loadFile()
        let document = try #require(files.selectedDocument)
        try #require(document.snapshot != nil)
        document.editor.insertText("Preserved draft", replacementRange: NSRange(location: 0, length: document.editor.string.utf16.count))
        document.editor.setSelectedRange(NSRange(location: 2, length: 3))
        var link = URLComponents()
        link.scheme = "loopflow"; link.host = "task"; link.path = "/" + fixture.issue
        link.queryItems = [URLQueryItem(name: "repo", value: fixture.repo), URLQueryItem(name: "session", value: fixture.sessionId)]
        let url = try #require(link.url)
        // PodiumView already owns keepWorkspaceCurrent. Starting another poller
        // here would invalidate the very idle measurement this test collects.
        let start = ContinuousClock.now
        var round = 0
        if seconds > 0 {
            try journal.write(["event": "soak_begin", "pid": ProcessInfo.processInfo.processIdentifier,
                               "seconds": seconds, "time": Date().timeIntervalSince1970])
        }
        repeat {
            if seconds > 0 {
                try journal.write(["event": "soak_phase", "phase": "navigation_typing", "round": round,
                                   "time": Date().timeIntervalSince1970])
            }
            try await sample("native_session_reopen", "snapshot", round, journal, window, soaking: round >= samples, action: {
                await model.openTaskLink(url)
            }, ready: {
                guard let surface = registry.surfaces.view(for: identity).surface else { return false }
                return model.selection?.id == fixture.taskId
                    && window.firstResponder === registry.surfaces.view(for: identity)
                    && terminalText(surface).contains("native:\(fixture.nativeId):retained-history")
            }, observation: {
                ["session_id": fixture.sessionId, "task_id": fixture.taskId, "native_id": fixture.nativeId,
                 "provider": "owned Codex stub", "history_preserved": (try? fixture.historyIsPreserved()) == true]
            }, input: {
                let surface = try #require(registry.surfaces.view(for: identity).surface)
                let message = "reopened-\(round)"
                try send(window, message + "\n")
                try await wait(window) { terminalText(surface).contains("reply:\(fixture.nativeId):\(message)") }
            })
            // Navigate away and back while the client is still live, retaining
            // both surfaces. The Task link selects the same recorded conversation.
            model.select(nil)
            await model.openTaskLink(url)
            try await wait(window) { window.firstResponder === registry.surfaces.view(for: identity) }
            await files.refresh()
            await files.loadFile()
            try #require(document.text == "Preserved draft")
            try #require(document.editor.selectedRange() == NSRange(location: 2, length: 3))
            try #require(multiplexer.layout == layout)
            try #require(companionView.surface == companionSurface)
            try #require(try fixture.historyIsPreserved())
            try send(window, "quit\n")
            try await wait(window) { !registry.surfaces.hasSurface(identity) }
            registry.surfaces.release(identity)
            store.noteSurfaceClosed(identity)
            let retained = try await fixture.records()
            try #require(retained.map(\.id) == [fixture.sessionId])
            try #require(retained[0].taskIds.contains(fixture.taskId))
            if seconds > 0 {
                try journal.write(["event": "soak_round", "round": round, "preserved": true,
                    "time": Date().timeIntervalSince1970, "windows": NSApp.windows.filter(\.isVisible).count,
                    "key_window": NSApp.keyWindow?.windowNumber ?? -1, "focused_session": fixture.sessionId])
                try journal.write(["event": "soak_phase", "phase": "idle", "round": round,
                                   "time": Date().timeIntervalSince1970])
                let remaining = seconds - Double(start.duration(to: .now).components.seconds)
                if remaining > 0 { try await Task.sleep(for: .seconds(min(30, remaining))) }
            }
            round += 1
        } while round < samples || start.duration(to: .now) < .seconds(seconds)
        if seconds > 0 {
            try journal.write(["event": "soak_end", "elapsed_seconds": Double(start.duration(to: .now).components.seconds),
                               "rounds": round, "preserved": true, "time": Date().timeIntervalSince1970])
            try await waitForResourceRecorder()
        }
    }

    private func waitForResourceRecorder() async throws {
        let finished = URL(fileURLWithPath: try #require(ProcessInfo.processInfo.environment["LF_DESKTOP_PERF_OUTPUT"]))
            .deletingLastPathComponent().appendingPathComponent("resources-finished")
        let deadline = ContinuousClock.now + .seconds(60)
        while !FileManager.default.fileExists(atPath: finished.path), ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(100))
        }
    }

    private func terminalText(_ surface: ghostty_surface_t) -> String {
        var text = ghostty_text_s()
        let selection = ghostty_selection_s(
            top_left: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_TOP_LEFT, x: 0, y: 0),
            bottom_right: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_BOTTOM_RIGHT, x: 0, y: 0), rectangle: false)
        guard ghostty_surface_read_text(surface, selection, &text) else { return "" }
        defer { ghostty_surface_free_text(surface, &text) }
        guard let bytes = text.text else { return "" }
        return String(decoding: Data(bytes: bytes, count: Int(text.text_len)), as: UTF8.self)
    }
#endif

    private func sample(_ scenario: String, _ population: String, _ attempt: Int,
                        _ journal: PerformanceJournal, _ window: PerformanceWindow,
                        soaking: Bool = false, action: () async throws -> Void, ready: () -> Bool,
                        observation: () -> [String: Any] = { [:] },
                        input: () async throws -> Void = {}) async throws {
        let metric = scenario == "new_pty" ? "new_pty_capture_and_echo_ms"
            : ["full", "fold", "expand", "compact", "sessions", "filter", "scroll_refresh"].contains(scenario)
                ? "hierarchy_interaction_ms" : "task_workspace_ready_ms"
        var record: [String: Any] = ["event": soaking ? "soak_sample_begin" : "begin", "id": "\(population)-\(scenario)-\(attempt)",
            "metric": metric, "scenario": scenario, "population": population, "attempt": attempt,
            "state": attempt == 0 ? "first_interaction" : "warm"]
        record["windows_before"] = NSApp.windows.filter(\.isVisible).count
        record["window_number"] = window.windowNumber
        record["focus_before"] = window.firstResponder.map { String(describing: type(of: $0)) } ?? "none"
        try journal.write(record)
        let start = DispatchTime.now().uptimeNanoseconds
        do {
            try await action()
            try await wait(window, render: true, ready: ready)
            let captured = Double(window.observedAt - start) / 1_000_000
            let verified = milliseconds(start)
            let inputStart = DispatchTime.now().uptimeNanoseconds
            try await input()
            if scenario == "session_return" { record["pty_echo_ms"] = milliseconds(inputStart) }
            record["outcome"] = "passed"
            record["capture_ready_ms"] = captured
            record["verification_ms"] = verified - captured
        } catch {
            record["outcome"] = (error as? PerformanceFailure)?.outcome ?? "failed"
            record["reason"] = String(describing: error)
            record["event"] = soaking ? "soak_sample_end" : "end"
            record["duration_ms"] = milliseconds(start)
            record["observation"] = observation()
            try journal.write(record)
            throw error
        }
        record["event"] = soaking ? "soak_sample_end" : "end"
        record["duration_ms"] = milliseconds(start)
        record["observation"] = observation()
        record["windows_after"] = NSApp.windows.filter(\.isVisible).count
        record["focus_after"] = window.firstResponder.map { String(describing: type(of: $0)) } ?? "none"
        try journal.write(record)
    }

    private func milliseconds(_ start: UInt64) -> Double {
        Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000
    }

    private func wait(_ window: PerformanceWindow, render: Bool = false, ready: () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(5)
        repeat {
            window.contentView?.layoutSubtreeIfNeeded()
            window.layoutIfNeeded()
            window.displayIfNeeded()
            if render { try window.capture() }
            if ready() { return }
            try await Task.sleep(for: .milliseconds(5))
        } while ContinuousClock.now < deadline
        throw PerformanceFailure("timeout", "Captured native content/input endpoint not reached in 5 seconds")
    }

    private func hasRendered(_ window: PerformanceWindow, _ id: String) -> Bool {
        // Identities are asserted against the shared model/pane owners; these
        // unique fixture labels independently prove the pixels changed too.
        if id == "workspace-wave-wave-1" { return window.outlineText.contains { $0.contains("product") } }
        if let index = Int(id.replacingOccurrences(of: "workspace-task-perf-task-", with: "")) {
            return window.outlineText.contains { $0.contains(String(format: "Task %03d", index)) }
        }
        if let index = Int(id.replacingOccurrences(of: "session-row-perf-session-", with: "")) {
            return window.outlineText.contains { $0.contains(String(format: "Conversation %03d", index)) }
        }
        if id.hasPrefix("monitor-session-") {
            return window.contentText.contains { $0.contains(String(id.dropFirst("monitor-session-".count))) }
        }
        if id.hasPrefix("monitor-empty-") {
            return window.contentText.joined(separator: " ").contains("No active Sessions in this observation")
        }
        return false
    }

    private func send(_ window: NSWindow, _ text: String) throws {
        for character in text {
            let event = try #require(NSEvent.keyEvent(
                with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0,
                windowNumber: window.windowNumber, context: nil, characters: String(character),
                charactersIgnoringModifiers: String(character), isARepeat: false,
                keyCode: character == "\n" ? 36 : 0))
            window.sendEvent(event)
        }
    }

    private func accessibility(_ element: NSObject, _ property: String) -> Any? {
        let key = "accessibility" + property
        guard element.responds(to: NSSelectorFromString(key)) else { return nil }
        return element.value(forKey: key)
    }

    private func accessible(_ root: Any) -> [NSObject] {
        guard let element = root as? NSObject else { return [] }
        return [element] + ((accessibility(element, "Children") as? [Any]) ?? []).flatMap { accessible($0) }
    }

    @discardableResult
    private func revealElement(_ value: String, in window: NSWindow, property: String = "Identifier") throws -> NSObject {
        let target = window.attachedSheet ?? window
        let content = try #require(target.contentView)
        let element = try #require(accessible(content).first { accessibility($0, property) as? String == value })
        if let frame = accessibility(element, "Frame") as? NSValue,
           let scroll = scrollView(in: content), let document = scroll.documentView {
            let rect = document.convert(target.convertFromScreen(frame.rectValue), from: nil)
            document.scrollToVisible(CGRect(x: rect.minX, y: rect.minY, width: 10, height: 40))
            scroll.reflectScrolledClipView(scroll.contentView)
        }
        return element
    }

    private func pressElement(_ value: String, in window: NSWindow, property: String = "Identifier") throws {
        let element = try revealElement(value, in: window, property: property)
        let action = NSSelectorFromString("accessibilityPerformPress")
        try #require(element.responds(to: action))
        element.perform(action)
    }

    private func scrollView(in view: NSView) -> NSScrollView? {
        if let scroll = view as? NSScrollView { return scroll }
        return view.subviews.lazy.compactMap { scrollView(in: $0) }.first
    }

    fileprivate func populationQuery(taskCount: Int, checkout: String = "/src/loopflow") throws -> (RegistryQuery, PerformancePlanning) {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let data = try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json"))
        var snapshot = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        var wave = try #require((snapshot["waves"] as? [[String: Any]])?.first)
        var tasks = try #require(wave["tasks"] as? [String: Any])
        let template = try #require((tasks["items"] as? [[String: Any]])?.first)
        tasks["items"] = try (0..<taskCount).map { index in
            var task = template
            var planning = try #require(task["task"] as? [String: Any])
            planning["id"] = "perf-task-\(index)"
            planning["identifier"] = "PERF-\(index)"
            planning["name"] = String(format: "Task %03d", index)
            planning["rank"] = Double(index)
            planning["completed"] = false
            task["task"] = planning
            var reference: [String: Any] = ["issue_url": NSNull(), "workspace": [
                "slug": "benchmark", "branch": "benchmark", "home_id": fixtureHomeId,
                "worktree": checkout, "local_exists": true,
            ]]
            if index == 1 {
                reference["workspace"] = ["slug": "benchmark-empty", "branch": "benchmark-empty", "home_id": fixtureHomeId,
                                          "worktree": NSTemporaryDirectory(), "local_exists": true]
            }
            task["reference"] = reference
            var runtime = try #require(task["runtime"] as? [String: Any])
            runtime["work_id"] = "perf-work-\(index)"
            runtime["started"] = true
            task["runtime"] = runtime
            return task
        }
        wave["tasks"] = tasks
        wave["unavailable_tasks"] = []
        var waveInfo = try #require(wave["wave"] as? [String: Any])
        waveInfo["repo"] = checkout
        wave["wave"] = waveInfo
        snapshot["waves"] = [wave]
        let roadmap = String(decoding: try JSONSerialization.data(withJSONObject: snapshot), as: UTF8.self)
        let planning = PerformancePlanning(roadmap: roadmap)
        let sessions = try JSONSerialization.data(withJSONObject: stride(from: 0, to: taskCount, by: 2).map { index in
            ["id": "perf-session-\(index)", "interactive": true, "kind": "conversation",
             "work": ["kind": "task", "id": "perf-work-\(index)"],
             "workspace": ["home_id": fixtureHomeId, "worktree": checkout,
                           "task_id": "perf-work-\(index)", "unavailable": NSNull()],
             "title": String(format: "Conversation %03d", index), "detail": "Benchmark fixture",
             "cwd": checkout, "wave_id": "wave-1", "state": "active", "ready_summary": NSNull(), "work_path": NSNull(),
             "actions": sessionActionFixture(kind: "conversation", state: "active"),
             "title_source": "generated", "flow_membership": ["kind": "independent"], "task_ids": ["perf-work-\(index)"], "terminal_ids": [], "open_argv": ["must-not-launch"]] as [String: Any]
        })
        let historyTemplate = try #require(JSONSerialization.jsonObject(with: Data(contentsOf:
            root.appendingPathComponent("tests/fixtures/dto/session_history_summary.json"))) as? [String: Any])
        let history = try JSONSerialization.data(withJSONObject: (0..<taskCount).map { index in
            var row = historyTemplate
            row["session_id"] = "history-\(index)"
            return row
        })
        let historyJSON = String(decoding: history, as: UTF8.self)
        let fixtureRoot = root.appendingPathComponent("tests/fixtures/dto")
        let workJSON = try String(contentsOf: fixtureRoot.appendingPathComponent("task_work.json"), encoding: .utf8)
        let commentsJSON = try String(contentsOf: fixtureRoot.appendingPathComponent("task_comments.json"), encoding: .utf8)
        let contextJSON = try String(contentsOf: fixtureRoot.appendingPathComponent("context_report.json"), encoding: .utf8)
        let catalogJSON = try String(contentsOf: fixtureRoot.appendingPathComponent("flow_catalog.json"), encoding: .utf8)
        let active = try JSONSerialization.data(withJSONObject: [
            "discovery": "ready", "home": "benchmark-fixture", "observed_at": 1790270400, "task": NSNull(), "gaps": [],
            "sessions": [0, 2].map { index in
                ["id": "perf-session-\(index)", "work": ["kind": "task", "id": "perf-work-\(index)"],
                 "title": "Fixture Session \(index)", "processes": [["pid": index + 1, "provider": "fixture", "state": "waiting"]]] as [String: Any]
            }
        ])
        let sessionJSON = String(decoding: sessions, as: UTF8.self)
        let activeJSON = String(decoding: active, as: UTF8.self)
        let feed = ActiveSessionsTestFeed()
        let query = RegistryQuery(watchActiveSessions: { try await feed.open(initial: activeJSON) }) { args, _ in
            await planning.count(args)
            switch args.first {
            case "home" where args.dropFirst().first == "id": return "{\"id\":\"\(fixtureHomeId)\"}"
            case "roadmap": return await planning.read()
            case "flow" where args.dropFirst().first == "list" && args.contains("--json"): return catalogJSON
            case "wave" where args.dropFirst().first == "list": return "[]"
            case "session" where args.dropFirst().first == "list": return #"{"entries":\#(sessionJSON),"next":null}"#
            case "usage": return args.contains("--context") ? contextJSON : historyJSON
            case "task" where args.dropFirst().first == "status": return "{\"work\":\(workJSON)}"
            case "task" where args.dropFirst().first == "comment" && args.contains("--json"): return commentsJSON
            case "task" where args.dropFirst().first == "files":
                return #"{"path":"","entries":[{"path":"notes.txt","kind":"file"}],"next_cursor":null}"#
            case "task" where args.dropFirst().first == "file":
                return #"{"issue_identifier":"PERF-0","task_id":"perf-work-0","path":"notes.txt","content":"Original notes","state":"text","revision":"fixed","size_bytes":14,"recoveries":[],"read_only_reason":null}"#
            case "activity": return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            default: throw RegistryQueryError("Benchmark does not launch providers or mutate planning")
            }
        }
        return (query, planning)
    }
}

@Suite("Desktop benchmark fixtures")
@MainActor
struct DesktopPerformanceFixtureTests {
    @Test func completeHistoryAndDraftRefreshUseOnlyFixtureTransport() async throws {
        let (query, _) = try DesktopPerformanceTests().populationQuery(taskCount: 256)
        let model = PodiumModel(query: query, repoPath: "/src/loopflow")
        await model.refresh()
        #expect(model.workspace.waves.first?.tasks.count == 256)
        #expect(model.sessions.value?.count == 128)
        let task = try #require(model.task(id: "perf-task-0"))
        #expect(task.task.reference.workspace?.identity == fixtureWorkspace("/src/loopflow"))
        #expect(model.sessions.value?.first?.workspace?.identity == task.task.reference.workspace?.identity)
        await model.loadFlowCatalog()
        await model.loadTaskWork(task: task.task, wave: task.wave.wave)
        await model.loadTaskContext(task: task.task, wave: task.wave.wave)
        #expect(model.flowCatalog.errorMessage == nil)
        #expect(model.taskWork[task.task.id].errorMessage == nil)
        #expect(model.taskContext[task.task.id].errorMessage == nil)
        await model.loadSessionHistory(task: task.task, wave: task.wave.wave)
        #expect(model.sessionHistory[task.task.id].value?.count == 256)
        #expect(model.sessionHistory[task.task.id].errorMessage == nil)
        let store = TaskFilesStore(issue: "PERF-0", cwd: NSTemporaryDirectory(), query: query)
        store.autosave = false
        store.selection = "notes.txt"
        await store.refresh()
        await store.loadFile()
        let document = try #require(store.selectedDocument)
        #expect(document.text == "Original notes")
        document.editor.insertText("Preserved draft", replacementRange: NSRange(location: 0, length: 14))
        await store.refresh()
        await store.loadFile()
        #expect(document.text == "Preserved draft")
        #expect(store.directories[""]?.entries.map(\.path) == ["notes.txt"])
        await #expect(throws: RegistryQueryError.self) {
            try await query.updateTaskDirective(id: "PERF-0", wave: "product", text: "denied", cwd: "/src/loopflow")
        }
    }
}

/// Hold the fixture's transport response to exercise scrolling while the real
/// Podium reader is refreshing. No product state is changed outside that reader.
@MainActor
fileprivate final class PerformancePlanning {
    let roadmap: String
    var reads: [String: Int] = [:]
    func count(_ args: [String]) { reads[args.prefix(2).joined(separator: " "), default: 0] += 1 }
    var holdNextRead = false
    var pending: CheckedContinuation<Void, Never>?

    init(roadmap: String) { self.roadmap = roadmap }

    func read() async -> String {
        guard holdNextRead else { return roadmap }
        holdNextRead = false
        await withCheckedContinuation { pending = $0 }
        return roadmap.replacingOccurrences(of: "Task ", with: "Updated ")
    }

    func release() {
        pending?.resume()
        pending = nil
    }
}

/// A test-owned window observes its actual AppKit bitmap. Recognition verifies
/// visible labels after capture; neither a model change nor onAppear ends a sample.
@MainActor
private final class PerformanceWindow: NSWindow {
    var outlineText: [String] = []
    var contentText: [String] = []
    var observedAt: UInt64 = 0
    var allText: [String] { outlineText + contentText }

    func capture() throws {
        guard let host = attachedSheet?.contentView ?? contentView,
              let bitmap = host.bitmapImageRepForCachingDisplay(in: host.bounds) else {
            throw PerformanceFailure("unavailable", "Native capture is unavailable")
        }
        host.cacheDisplay(in: host.bounds, to: bitmap)
        guard let image = bitmap.cgImage else {
            throw PerformanceFailure("unavailable", "Native bitmap has no image")
        }
        observedAt = DispatchTime.now().uptimeNanoseconds
        let request = VNRecognizeTextRequest()
        request.recognitionLevel = .fast
        request.recognitionLanguages = ["en-US"]
        request.usesLanguageCorrection = false
        try VNImageRequestHandler(cgImage: image).perform([request])
        let rows = request.results ?? []
        let outlineWidth = 300 / host.bounds.width
        outlineText = rows.filter { $0.boundingBox.midX < outlineWidth }.compactMap { $0.topCandidates(1).first?.string }
        contentText = rows.filter { $0.boundingBox.midX >= outlineWidth }.compactMap { $0.topCandidates(1).first?.string }
    }
}

private struct PerformanceFailure: Error, CustomStringConvertible {
    let outcome: String
    let description: String
    init(_ outcome: String, _ description: String) { self.outcome = outcome; self.description = description }
}

@MainActor
private final class PerformanceJournal {
    private let file: FileHandle
    init(url: URL) throws {
        guard FileManager.default.createFile(atPath: url.path, contents: nil) else {
            throw PerformanceFailure("failed", "Cannot create benchmark journal")
        }
        file = try FileHandle(forWritingTo: url)
    }
    deinit { try? file.close() }
    func write(_ record: [String: Any]) throws {
        var data = try JSONSerialization.data(withJSONObject: record, options: [.sortedKeys])
        data.append(10)
        try file.write(contentsOf: data)
        try file.synchronize()
    }
}

extension DesktopPerformanceTests {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["LF_DESKTOP_TASK_SNAPSHOT"] != nil))
    func measureSnapshotTaskOpening() async throws {
        let env = ProcessInfo.processInfo.environment
        let snapshot = try #require(env["LF_DESKTOP_TASK_SNAPSHOT"])
        let binary = try #require(env["LF_DESKTOP_TASK_BINARY"])
        let repo = try #require(env["LF_DESKTOP_TASK_REPO"])
        let issue = try #require(env["LF_DESKTOP_TASK_ISSUE"])
        let samples = Int(env["LF_DESKTOP_PERF_SAMPLES"] ?? "21") ?? 21
        let journal = try PerformanceJournal(url: URL(fileURLWithPath: try #require(env["LF_DESKTOP_PERF_OUTPUT"])))
        let scenarios = ["cold_workspace", "warm_task", "reopen_task"]
        let fixture = try DesktopNativeSessionFixture.load()
        let soakSeconds = Double(env["LF_DESKTOP_PERF_SOAK_SECONDS"] ?? "0") ?? 0
        try journal.write(["event": "plan", "population_version": "home-snapshot-v2",
                           "populations": ["snapshot": 1], "samples": samples,
                           "scenarios": scenarios + ["native_session_reopen"], "soak_seconds": soakSeconds,
                           "endpoint": "native_capture_ocr_and_pty_reply", "poll_interval_ms": 5,
                           "gaps": ["A new Podium and native window measure cold workspace construction, not OS application launch.",
                                    "Bitmap capture is not compositor presentation; OCR adds observer cost.",
                                    "Copied providers cannot connect; native usability uses an owned Task and synthetic provider."]])
        _ = NSApplication.shared
        NSApp.setActivationPolicy(.accessory)
        NSApp.finishLaunching()
        guard NSScreen.main != nil else {
            try journal.write(["event": "setup", "outcome": "unavailable", "reason": "No native screen"])
            throw PerformanceFailure("unavailable", "No native screen")
        }
#if canImport(GhosttyKit)
        GhosttyManager.shared.initialize()
        try #require(GhosttyManager.shared.state == .ready)
#endif
        var components = URLComponents()
        components.scheme = "loopflow"
        components.host = "task"
        components.path = "/" + issue
        components.queryItems = [URLQueryItem(name: "repo", value: repo)]
        let url = try #require(components.url)
        for attempt in 0..<samples {
            let reads = SnapshotReads()
            let query = RegistryQuery { args, cwd in
                let start = DispatchTime.now().uptimeNanoseconds
                let result = await reads.read(binary: binary, home: snapshot, args: args, cwd: cwd ?? repo, fixture: fixture)
                try await MainActor.run {
                    try journal.write(["event": "read", "sample": attempt, "args": args,
                                       "duration_ms": Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000,
                                       "outcome": result.isSuccess ? "passed" : "unavailable"])
                }
                return try result.get()
            }
            let router = WorkspaceLinkRouter()
            let view = PodiumView(portfolioService: PortfolioService(), initialRepoPath: repo,
                                  query: query, taskLinks: router)
            let window = PerformanceWindow(contentRect: CGRect(x: 0, y: 0, width: 1280, height: 800),
                                           styleMask: [.titled, .resizable], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            defer { window.contentView = nil; window.close() }
            var model: PodiumModel?
            for scenario in scenarios {
                let start = DispatchTime.now().uptimeNanoseconds
                var record: [String: Any] = ["event": "begin", "id": "snapshot-\(scenario)-\(attempt)",
                    "metric": "task_workspace_ready_ms", "scenario": scenario,
                    "population": "snapshot", "attempt": attempt,
                    "state": attempt == 0 ? "first_interaction" : "warm"]
                try journal.write(record)
                var transitions: [[String: Any]] = []
                var lastState = ""
                do {
                    if scenario == "cold_workspace" {
                        router.deliver(url)
                        window.contentView = NSHostingView(rootView: view)
                        window.makeKeyAndOrderFront(nil)
                    } else {
                        if scenario == "warm_task" { model?.select(nil) }
                        router.deliver(url)
                    }
                    // The receiver schedules its async resolution on the main actor.
                    // Do not accept the previous destination before that request starts.
                    try await Task.sleep(for: .milliseconds(5))
                    let deadline = ContinuousClock.now + .seconds(45)
                    var ready = false
                    var blocked = "workspace model not found"
                    repeat {
                        window.contentView?.layoutSubtreeIfNeeded()
                        window.displayIfNeeded()
                        if model == nil { model = try? view.inspect().find(SessionsView.self).actualView().model }
                        let visible = NSApp.windows.filter(\.isVisible)
                        let sheet = model?.showsTaskLink ?? false
                        let selected = model?.selection?.id ?? ""
                        let state = "\(visible.map(\.windowNumber).sorted())/\(NSApp.keyWindow?.windowNumber ?? -1)/\(sheet)/\(selected)"
                        if state != lastState {
                            transitions.append(["at_ms": milliseconds(start), "window_numbers": visible.map(\.windowNumber),
                                                "key_window": NSApp.keyWindow?.windowNumber ?? -1,
                                                "task_link_sheet": sheet, "selection": selected])
                            lastState = state
                        }
                        if let model {
                            if model.selection.flatMap({ model.task(id: $0.id) })?.task.task.identifier != issue {
                                blocked = "selection is not the Task"
                            } else if model.taskLinkReading.isLoading || sheet {
                                blocked = sheet ? "Task-link sheet shown" : "Task link still resolving"
                            } else {
                                try window.capture()
                                // Fast recognition splits the breadcrumb's mono identifier ("LOO- 368").
                                ready = (window.contentText + window.outlineText).contains {
                                    $0.filter { !$0.isWhitespace }.contains(issue)
                                }
                                if ready { break }
                                blocked = "captured content lacks the issue: \(window.contentText.prefix(24))"
                            }
                        }
                        try await Task.sleep(for: .milliseconds(5))
                    } while ContinuousClock.now < deadline
                    guard ready else { throw PerformanceFailure("timeout", "Task identity not captured in 45 seconds: \(blocked)") }
                    record["outcome"] = "passed"
                    record["capture_ready_ms"] = Double(window.observedAt - start) / 1_000_000
                } catch {
                    record["outcome"] = (error as? PerformanceFailure)?.outcome ?? "failed"
                    record["reason"] = String(describing: error)
                }
                record["event"] = "end"
                record["duration_ms"] = milliseconds(start)
                record["observation"] = ["transitions": transitions, "selected_session": model?.navigation.selectedSessionId as Any? ?? NSNull(),
                                         "session_usable_ms": NSNull()]
                try journal.write(record)
            }
#if canImport(GhosttyKit)
            if attempt == samples - 1 {
                let sessions = try view.inspect().find(SessionsView.self).actualView()
                try await measureOwnedSoak(view: sessions, window: window, fixture: fixture, query: query,
                                           samples: samples, seconds: soakSeconds, journal: journal)
            }
#else
            throw PerformanceFailure("unavailable", "GhosttyKit is absent")
#endif
            window.contentView = nil
            window.close()
            // Disappearing SwiftUI views cancel their pollers, but detached CLI
            // reads must finish before another sample constructs its workspace.
            await reads.finish()
        }
    }
}

actor SnapshotReads {
    private var active = 0
    private var closed = false

    func read(binary: String, home: String, args: [String], cwd: String, fixture: DesktopNativeSessionFixture? = nil) async -> Result<String, Error> {
        guard !closed else { return .failure(CancellationError()) }
        active += 1
        defer { active -= 1 }
        do {
            let verb = args.prefix(2).joined(separator: " ")
            if let fixture, verb == "session connect" || verb == "home id"
                || args.contains(fixture.issue) || args.contains(fixture.taskId) {
                return .success(try await fixture.read(args))
            }
            let output = try await Task.detached {
                try snapshotRead(binary: binary, home: home, args: args, cwd: cwd)
            }.value
            guard let fixture else { return .success(output) }
            var value = try JSONSerialization.jsonObject(with: Data(output.utf8))
            if args.first == "roadmap", !args.contains("--task") {
                let owned = try await fixture.read(["roadmap", "--all", "--json"])
                var page = try #require(value as? [String: Any])
                let extra = try #require(JSONSerialization.jsonObject(with: Data(owned.utf8)) as? [String: Any])
                page["waves"] = (try #require(page["waves"] as? [Any])) + (try #require(extra["waves"] as? [Any]))
                value = page
            } else if verb == "wave list" {
                let owned = try await fixture.read(args)
                value = (try #require(value as? [Any])) + (try #require(JSONSerialization.jsonObject(with: Data(owned.utf8)) as? [Any]))
            } else if verb == "session list" {
                // The synthetic conversation remains explicitly discoverable after
                // client exit. Copied entries and their paging cursor stay intact.
                if let page = value as? [String: Any], !(page["next"] is NSNull) { return .success(output) }
                let owned = try await fixture.read(["session", "list", "--all", "--history", "--json"])
                let extra = try #require(JSONSerialization.jsonObject(with: Data(owned.utf8)) as? [Any])
                if var page = value as? [String: Any] {
                    if page["next"] is NSNull { page["entries"] = (try #require(page["entries"] as? [Any])) + extra }
                    value = page
                } else { value = (try #require(value as? [Any])) + extra }
            }
            return .success(String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self))
        } catch { return .failure(error) }
    }

    func finish() async {
        closed = true
        while active > 0 { try? await Task.sleep(for: .milliseconds(10)) }
    }
}

private func snapshotRead(binary: String, home: String, args: [String], cwd: String) throws -> String {
    // Copied launch authority is never exercised. This transport permits only local
    // reads; no network, Session connection, watcher, worker or provider can start.
    let verb = args.prefix(2).joined(separator: " ")
    guard args.first == "roadmap" || ["wave list", "wave status", "session list", "home id", "task files", "task diff"].contains(verb) else {
        throw RegistryQueryError("Snapshot does not execute \(verb)")
    }
    let process = Process()
    process.executableURL = URL(fileURLWithPath: binary)
    process.arguments = args
    process.currentDirectoryURL = URL(fileURLWithPath: cwd)
    let git = ProcessInfo.processInfo.environment["LOOPFLOW_TEST_GIT"] ?? "/usr/bin/git"
    var environment = ["HOME": home, "PATH": URL(fileURLWithPath: git).deletingLastPathComponent().path + ":/usr/bin:/bin", "TMPDIR": home, "RUST_LOG": "off"]
    environment["LF_HOME"] = home
    environment["LF_PERF_OUTPUT"] = ProcessInfo.processInfo.environment["LF_PERF_OUTPUT"]
    environment["GIT_OPTIONAL_LOCKS"] = "0"
    process.environment = environment
    let output = Pipe()
    process.standardOutput = output
    process.standardError = FileHandle.standardError
    try process.run()
    let timeout = DispatchWorkItem { if process.isRunning { process.terminate() } }
    DispatchQueue.global().asyncAfter(deadline: .now() + 30, execute: timeout)
    defer { timeout.cancel() }
    let data = output.fileHandleForReading.readDataToEndOfFile()
    process.waitUntilExit()
    guard process.terminationStatus == 0 else { throw RegistryQueryError("Snapshot read failed: \(verb) (\(process.terminationStatus))") }
    return String(decoding: data, as: UTF8.self)
}

private extension Result {
    var isSuccess: Bool { if case .success = self { return true }; return false }
}
#endif
