#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
import ViewInspector
#if canImport(GhosttyKit)
import GhosttyKit
#endif
@testable import Loopflow
@testable import LoopflowMac

private let fixtureRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    .deletingLastPathComponent().deletingLastPathComponent()
    .appendingPathComponent("tests/fixtures/dto")

private func fixture(_ name: String) throws -> Data {
    try Data(contentsOf: fixtureRoot.appendingPathComponent(name))
}

@Suite("Task Flow")
struct TaskFlowTests {
    @Test("Participation opens only the captured occurrence in the current pass")
    func exactParticipationSession() throws {
        let snapshots = try JSONDecoder().decode([TaskFlowSnapshot].self, from: fixture("task_flow.json"))
        guard case .latest(let latest) = snapshots[2].record else { Issue.record("latest"); return }
        var value = try #require(JSONSerialization.jsonObject(with: fixture("session.json")) as? [String: Any])
        value["kind"] = "flow"
        value["id"] = "exact"
        value["state"] = "waiting"
        value["flow_membership"] = ["kind": "step", "flow": "feature", "invocation_id": latest.invocationId,
            "step": "demo", "node": 4, "iterations": latest.iterations, "occurrence": "current"]
        let exact = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: value))
        var wrong = value
        wrong["id"] = "previous"
        var membership = try #require(wrong["flow_membership"] as? [String: Any])
        membership["iterations"] = [[0, 0]]
        wrong["flow_membership"] = membership
        let previous = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: wrong))
        #expect(participationSession(node: "4", latest: latest, sessions: [previous, exact])?.id == "exact")
        #expect(participationSession(node: "4", latest: latest, sessions: [previous]) == nil)
        #expect(participationSession(node: "7", latest: latest, sessions: [exact]) == nil)
        #expect(exact.offersParticipation)
        #expect(exact.participationLabel == "Available · preparing")
        value["kind"] = "conversation"
        let independent = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: value))
        #expect(!independent.offersParticipation)
        value["kind"] = "flow"
        value["actions"] = []
        let failed = try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: value))
        #expect(!failed.offersParticipation)
        #expect(failed.participationLabel == "Needs recovery")
    }

    @Test("Flow snapshots and the catalogue decode every record without defaults")
    func flowFixtures() throws {
        let snapshots = try JSONDecoder().decode([TaskFlowSnapshot].self, from: fixture("task_flow.json"))
        #expect(snapshots.map(\.recommended) == ["feature", "feature", "feature", "build", "feature"])
        guard case .latest(let running) = snapshots[1].record else {
            Issue.record("second snapshot has a latest Flow"); return
        }
        #expect(running.returns.map(\.traversals) == [2, 0])
        #expect(running.returns.map(\.decider) == [3, 5])
        #expect(running.graph.steps.filter { $0.returnsTo == 1 }.map(\.key) == [3, 5])
        // Start stays legal beside an earlier Flow; it launches a fresh one.
        #expect(snapshots.allSatisfy { $0.controls == [TaskFlowControl(kind: .start, unavailable: nil)] })
        #expect(snapshots[4].record == .finished(flow: "build"))

        var missing = try #require(JSONSerialization.jsonObject(with: fixture("task_flow.json")) as? [[String: Any]])
        let record = try #require(missing[1]["record"] as? [String: Any])
        var stringNode = record
        stringNode["current"] = "2"
        missing[1]["record"] = stringNode
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode([TaskFlowSnapshot].self, from: JSONSerialization.data(withJSONObject: missing))
        }
        for field in ["returns", "iterations"] {
            var incomplete = record
            incomplete.removeValue(forKey: field)
            missing[1]["record"] = incomplete
            #expect(throws: DecodingError.self) {
                try JSONDecoder().decode([TaskFlowSnapshot].self, from: JSONSerialization.data(withJSONObject: missing))
            }
        }

        let catalog = try JSONDecoder().decode([FlowCatalogEntry].self, from: fixture("flow_catalog.json"))
        #expect(catalog.map(\.name) == ["feature", "broken"])
        #expect(catalog[0].graph?.steps.count == 8 && catalog[1].unavailable != nil)
        #expect(catalog[0].graph?.steps[0].sources == ["feature"])
        let graph = try #require(catalog[0].graph)
        let template = try #require(catalog[0].template)
        #expect(FlowTemplateProjection(graph: graph, items: template.items,
                                       expanded: ["group-0"]).graph == graph)
    }

    @Test("Template disclosure keeps repeated and empty groups, XOR paths and both returns")
    @MainActor
    func templateDisclosure() throws {
        let entry = try JSONDecoder().decode(FlowCatalogEntry.self, from: fixture("flow_template.json"))
        let template = try #require(entry.template)
        let graph = try #require(entry.graph)
        let folded = FlowTemplateProjection(graph: graph, items: template.items, expanded: [])
        #expect(folded.graph.steps.map(\.key) == [8, 9, 10, 2, 12])
        #expect(folded.graph.steps[0].label == folded.graph.steps[1].label)
        #expect(folded.graph.steps[2].label.contains("0"))
        let returns = FlowTemplateView.spans(graph, projection: folded)
        #expect(returns.map(\.decider) == [5, 7])
        #expect(returns.allSatisfy { $0.from == 4 && $0.to == 4 })
        let partial = FlowTemplateProjection(graph: graph, items: template.items, expanded: ["group-0", "group-4"])
        #expect(partial.graph.steps.map(\.key) == [0, 8, 9, 2, 4, 5, 6, 7])
        let all = FlowTemplateProjection(graph: graph, items: template.items, expanded: Set((0...4).map { "group-\($0)" }))
        #expect(all.graph == graph)
        #expect(all.graph.node(3)?.label == "implement")

        var missing = try #require(JSONSerialization.jsonObject(with: fixture("flow_template.json")) as? [String: Any])
        var body = try #require(missing["template"] as? [String: Any])
        body.removeValue(forKey: "items")
        missing["template"] = body
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(FlowCatalogEntry.self, from: JSONSerialization.data(withJSONObject: missing))
        }
    }

    @Test("Occurrence state keeps pass completions while iteration keeps each edge count")
    func occurrenceStates() throws {
        let snapshots = try JSONDecoder().decode([TaskFlowSnapshot].self, from: fixture("task_flow.json"))
        guard case .latest(let review) = snapshots[2].record else { Issue.record("latest"); return }
        let states = flowNodeStates(review.graph, latest: review)
        #expect(states[1] == .completed && states[3] == .completed)
        #expect(states[4] == .stopped)
        // The second decision is pending again in this pass; the router pends too.
        #expect(states[5] == .pending && states[6] == .pending)
        #expect(states[7] == .pending)
        #expect(review.iterations == [[1, 1]])
        #expect(review.returns[1].traversals == 1)
        guard case .latest(let running) = snapshots[1].record else { Issue.record("latest"); return }
        #expect(running.iterations == [[2, 0]])
        #expect(flowIterationLabel([[2, 1], [3]]) == "(2, 1) / (3)")
        #expect(flowIterationLabel([[]]) == nil)
        #expect(running.returns.map(\.traversals) == [2, 0])

        guard case .latest(let blocked) = snapshots[3].record else { Issue.record("latest"); return }
        #expect(flowNodeStates(blocked.graph, latest: blocked)[3] == .blocked)
        let stalledSnapshot = try JSONDecoder().decode(TaskFlowSnapshot.self, from: fixture("task_flow_stalled.json"))
        guard case .latest(let stalled) = stalledSnapshot.record else { Issue.record("latest"); return }
        #expect(stalled.execution == .stalled)
        #expect(flowNodeStates(stalled.graph, latest: stalled)[0] == .stalled)
        #expect(stalled.reason.contains("Session event 12"))
        // A preview only marks human boundaries.
        let preview = flowNodeStates(review.graph, latest: nil)
        #expect(preview[4] == .pendingHuman && preview[1] == .pending)
    }

    @Test("Captured numeric IDs preserve nested containment and independent return counts")
    func nestedNumericIdentity() throws {
        let snapshot = try JSONDecoder().decode(TaskFlowSnapshot.self, from: fixture("flow_numeric_nested.json"))
        guard case .latest(let latest) = snapshot.record else { Issue.record("latest"); return }
        let graph = latest.graph
        #expect(graph.steps.map(\.key) == [0, 1, 9])
        #expect(graph.node(5)?.label == "check")
        #expect(graph.node(5)?.returnsTo == 4)
        #expect(graph.node(6)?.returnsTo == 2)
        #expect(graph.node(8)?.returnsTo == 7)
        #expect(graph.node(9)?.returnsTo == 0)
        #expect(graph.node(10) == nil)
        let outer = try #require(graph.node(1))
        #expect(outer.contains(5) && outer.contains(8) && !outer.contains(9))
        #expect(outer.paths[0].steps.contains { $0.contains(5) })
        #expect(!outer.paths[1].steps.contains { $0.contains(5) })
        let states = flowNodeStates(graph, latest: latest)
        #expect(states[1] == .running && states[3] == .running && states[5] == .running)
        #expect(states[4] == .completed && states[7] == .pending && states[9] == .pending)
        #expect(latest.returns.map(\.decider) == [5, 6, 8, 9])
        #expect(latest.returns.map(\.traversals) == [2, 0, 0, 3])
        #expect(latest.iterations == [[3], [0], [2]])
    }

    @Test("Running status elapsed time and the two return ports")
    @MainActor
    func statusLineAndPorts() {
        let now = Date(timeIntervalSince1970: 1_800_000_000)
        #expect(TaskFlowView.elapsed(since: "2027-01-15T08:00:00Z", now: now) == "0s")
        #expect(TaskFlowView.elapsed(since: "2027-01-15T07:59:18Z", now: now) == "42s")
        #expect(TaskFlowView.elapsed(since: "2027-01-15T07:48:00Z", now: now) == "12m")
        #expect(TaskFlowView.elapsed(since: "2027-01-15T04:55:00Z", now: now) == "3h 05m")
        #expect(TaskFlowView.elapsed(since: "2027-01-13T05:00:00.000Z", now: now) == "2d 03h")
        #expect(TaskFlowView.elapsed(since: "yesterday", now: now) == nil)
        // Loop 1 lands from above, Loop 2 from below, both at the target's left edge.
        #expect(FlowLoopGeometry.returnsAbove(0) && !FlowLoopGeometry.returnsAbove(1))
        #expect(FlowLoopGeometry.landing(0) == FlowLoopGeometry.landing(1))
        #expect(FlowLoopGeometry.above(1) > FlowLoopGeometry.above(0))
        #expect(FlowLoopGeometry.below(1) > FlowLoopGeometry.below(0))
    }
}

#if canImport(GhosttyKit)
@Suite("Task Flow native proof", .requiresDisplay, .serialized)
@MainActor
struct TaskFlowProofTests {
    @Test("Flow preview, Start and execution updates keep the Session's terminal, draft and companion")
    func flowControlsRetainTerminals() async throws {
        _ = NSApplication.shared
        NSApp.accessibilitySetValue(true, forAttribute: NSAccessibility.Attribute(rawValue: "AXEnhancedUserInterface"))
        GhosttyManager.shared.initialize()
        let repo = "/src/loopflow"
        let registry = SessionsWorkspaceRegistry(localHomeId: fixtureHomeId)
        let workspace = registry.workspace(for: fixtureWorkspace(repo))
        var terminals: [GhosttyMetalView] = []
        for _ in 0..<2 {
            workspace.multiplexer.newShell()
            let pane = workspace.multiplexer.focusedPaneId
            let terminal = registry.surfaces.view(for: .shell(pane))
            terminal.frame = CGRect(x: 0, y: 0, width: 400, height: 350)
            terminal.workingDirectory = NSTemporaryDirectory()
            terminal.command = buildWorkspaceShellCommand(id: pane, argv: ["/bin/cat"], env: [:])
            terminal.createSurface(manager: GhosttyManager.shared)
            terminals.append(terminal)
        }
        defer { for terminal in terminals { registry.surfaces.release(terminal.terminal) } }
        let surfaces = try terminals.map { try #require($0.surface) }
        let shells = workspace.multiplexer.layout.allPanes.map(\.id)
        let layout = workspace.multiplexer.layout

        let reviewed = WorkReference.task(id: "ts_review00000000000000000000000000")
        var session = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(
            renameFixtureRecord("design", title: "review-design", work: reviewed)
        )) as? [String: Any])
        session["state"] = "active"
        session["actions"] = sessionActionFixture(kind: "conversation", state: "active")
        session["terminal_ids"] = [shells[0]]
        session["open_argv"] = ["must-not-launch"]
        let source = try FlowSource(session: JSONSerialization.data(withJSONObject: [session]))
        let query = RegistryQuery { args, _ in try await source.respond(args) }
        let model = PodiumModel(query: query, repoPath: repo)
        await model.refresh()
        model.navigation.content = .terminals
        let view = SessionsView(model: model, repoPath: repo, workspaces: registry, query: query)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1500, height: 820),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = NSHostingView(rootView: view)
        window.orderFront(nil)
        defer { window.orderOut(nil); window.contentView = nil }
        try await settle(window)
        let draft = "flow-proof-draft"
        draft.withCString { ghostty_surface_text(surfaces[0], $0, UInt(draft.utf8.count)) }

        func find(_ id: String) throws -> InspectableView<ViewType.ClassifiedView> {
            do {
                return try view.inspect().find(viewWithAccessibilityIdentifier: id)
            } catch {
                throw RegistryQueryError("Missing \(id): \(error)")
            }
        }
        func text(_ id: String) throws -> String { try find(id).text().string() }

        // Unstarted: the recommendation is previewed with both return edges.
        model.select(.task(id: "issue-available"))
        model.navigation.content = .details
        for _ in 0..<20 where model.flowCatalog.value == nil { try await settle(window) }
        try await settle(window)
        #expect(try text("task-flow-status") == "Not started · No runs yet.")
        #expect(try find("task-flow-loop-3").text().string() == "Loop 1")
        #expect(try find("task-flow-loop-5").text().string() == "Loop 2")
        #expect((try? find("task-flow-iteration")) == nil, "a preview has no iteration")
        #expect((try? find("flow-node-4")) == nil, "composition starts folded")
        try find("flow-node-9").button().tap()
        try await settle(window)
        #expect(try find("flow-node-4").accessibilityLabel().string() == "demo, human review, pending")
        try find("template-group-group-0").disclosureGroup().collapse()
        try await settle(window)
        #expect((try? find("flow-node-4")) == nil)
        #expect(await source.controls.isEmpty, "disclosure never starts work")

        // The Wave uses the current Project's template and shares disclosure state.
        model.select(.wave(id: "wave-1"))
        try await settle(window)
        _ = try find("flow-template-feature")
        #expect((try? find("task-flow-start")) == nil)
        try find("template-group-group-0").disclosureGroup().expand()
        try await settle(window)
        model.select(.task(id: "issue-available"))
        try await settle(window)
        #expect(try find("flow-node-4").accessibilityLabel().string() == "demo, human review, pending")


        let oldRevision = try #require(model.flowCatalog.value?.first?.template?.revision)
        try await source.reviseTemplate()
        await model.loadFlowCatalog(force: true)
        try await settle(window)
        #expect(model.flowCatalog.value?.first?.template?.revision != oldRevision)
        #expect((try? find("flow-node-4")) == nil, "a new source revision starts folded")
        #expect(await source.controls.isEmpty)

        // A return outside a folded composition still names its semantic target.
        try await source.crossingReturns()
        await model.loadFlowCatalog(force: true)
        try await settle(window)
        let inputBeforeInspection = surfaces.map(terminalText)
        #expect(inputBeforeInspection[0].contains(draft))
        #expect(terminals.allSatisfy { !$0.acceptsFirstResponder })
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(window.contentView)
        for _ in 0..<30 where focusedLabel(in: window) != "build · 1 steps" {
            try press("\t", keyCode: 48, in: window)
            try await settle(window)
        }
        try #require(focusedLabel(in: window) == "build · 1 steps")
        for prefix in ["", "3/fix/"] {
            if !prefix.isEmpty {
                try press("\t", keyCode: 48, in: window)
                try await settle(window)
                #expect(focusedLabel(in: window) == "xor-route · fix")
                try press("\u{f703}", keyCode: 124, in: window)
                try await settle(window)
                try press("\t", keyCode: 48, in: window)
                try await settle(window)
            }
            #expect(focusedLabel(in: window) == "build · 1 steps")
            func expanded(_ suffix: String) -> Bool {
                model.navigation.expandedTemplateGroups["crossing-returns"]?.contains(prefix + suffix) == true
            }
            try press("\u{f703}", keyCode: 124, in: window)
            try await settle(window)
            #expect(expanded("outer"))
            try press("\t", keyCode: 48, in: window)
            try await settle(window)
            #expect(focusedLabel(in: window) == "edit · 1 steps")
            try press(" ", keyCode: 49, in: window)
            try await settle(window)
            #expect(expanded("inner"))
            try press("\t", keyCode: 48, in: window)
            try await settle(window)
            #expect(focusedLabel(in: window) == "empty · 0 steps")
            try press("\r", keyCode: 36, in: window)
            try await settle(window)
            #expect(expanded("empty"))
            #expect(try find("template-group-\(prefix)empty").find(text: "No steps").string() == "No steps")
            try press("\u{f702}", keyCode: 123, in: window)
            try await settle(window)
            #expect(!expanded("empty"))
            try press("\u{19}", keyCode: 48, modifiers: [.shift], in: window)
            try await settle(window)
            #expect(focusedLabel(in: window) == "edit · 1 steps")
            try press("\u{f702}", keyCode: 123, in: window)
            try await settle(window)
            #expect(!expanded("inner"))
            try press("\u{19}", keyCode: 48, modifiers: [.shift], in: window)
            try await settle(window)
            #expect(focusedLabel(in: window) == "build · 1 steps")
            try press("\u{f702}", keyCode: 123, in: window)
            try await settle(window)
            #expect(!expanded("outer"))
        }
        for prefix in ["", "3/fix/"] {
            for expanded in [false, true, false] {
                let group = try find("template-group-\(prefix)outer").disclosureGroup()
                if expanded {
                    try group.expand()
                    try await settle(window)
                    try find("template-group-\(prefix)inner").disclosureGroup().expand()
                } else { try group.collapse() }
                try await settle(window)
                for (number, offset) in [1, 2].enumerated() {
                    let key = (prefix.isEmpty ? 0 : 4) + offset
                    #expect(try text("task-flow-loop-\(key)") == "Loop \(number + 1)")
                    try pressElement("flow-node-\(key)", in: window)
                    try await settle(window)
                    let expected = prefix.isEmpty ? "implement" : "fix-implement"
                    let details = accessible(window.contentView!).compactMap { ax($0, "Value") as? String }
                        .filter { $0.hasPrefix("Iterate returns to") }
                    #expect(details.contains("Iterate returns to \(expected)"))
                    #expect(details.allSatisfy { $0 == "Iterate returns to implement" || $0 == "Iterate returns to fix-implement" })
                }
            }
        }
        #expect(await source.controls.isEmpty, "template inspection never starts work")
        #expect(surfaces.map(terminalText) == inputBeforeInspection, "disclosure keys never reach a PTY")

        // Started alone does not establish historical Run membership.
        model.select(.task(id: "issue-later"))
        try await settle(window)
        #expect(try text("task-flow-status") == "No Flow recorded")
        _ = try find("flow-template-feature")
        #expect((try? find("task-flow-iteration")) == nil)
        #expect(await source.controls.isEmpty)
        model.select(.task(id: "issue-available"))
        try await settle(window)

        // Typeahead: Cancel keeps the recommendation; choosing previews only.
        try find("task-flow-name").button().tap()
        try await settle(window)
        try find("task-flow-search").textField().setInput("bu")
        try await settle(window)
        #expect(throws: (any Error).self) { try find("task-flow-option-feature") }
        try find("task-flow-search-cancel").button().tap()
        try await settle(window)
        #expect(throws: (any Error).self) { try find("task-flow-search") }
        #expect(try find("task-flow-name").accessibilityLabel().string() == "Flow feature, choose another")
        try find("task-flow-name").button().tap()
        try await settle(window)
        #expect(try find("task-flow-option-broken").button().isDisabled())
        try find("task-flow-option-build").button().tap()
        try await settle(window)
        #expect(try find("task-flow-name").accessibilityLabel().string() == "Flow build, choose another")
        #expect(await source.controls.isEmpty, "choosing a preview mutates nothing")
        try find("task-flow-start").button().tap()
        for _ in 0..<20 where await source.controls.isEmpty { try await settle(window) }
        #expect(await source.controls == [["--task", "W2-156", "flow", "start", "build"]])

        // A launched Flow stopped at review: the saved position, not the catalogue.
        model.select(.task(id: "issue-review"))
        try await settle(window)
        #expect(try text("task-flow-status") == "Stopped · Waiting for your review at demo")
        #expect(try find("flow-node-4").accessibilityLabel().string() == "demo, stopped here")
        #expect(try find("flow-node-1").accessibilityLabel().string() == "implement, completed this pass")
        // Each loop labels its own returns; the header carries the tuple.
        #expect(try find("task-flow-loop-3").text().string() == "Loop 1 · 1 return")
        #expect(try find("task-flow-loop-5").text().string() == "Loop 2 · 1 return")
        #expect(try text("task-flow-iteration") == "Iteration (1, 1)")
        #expect(try !find("task-flow-start").button().isDisabled(), "Start stays offered beside the latest Flow")
        for key in ["3", "5"] {
            try pressElement("flow-node-\(key)", in: window)
            try await settle(window)
            let details = accessible(window.contentView!).compactMap { ax($0, "Value") as? String }
            #expect(details.contains { $0.contains("Iterate returns to implement · taken 1×") })
        }
        try captureIfRequested(window, name: "task-flow-latest")

        // Choosing another Flow beside the latest one mutates nothing; a refused
        // Start keeps the latest Flow drawn and names the failure.
        try find("task-flow-name").button().tap()
        try await settle(window)
        try find("task-flow-option-build").button().tap()
        try await settle(window)
        #expect(try find("task-flow-name").accessibilityLabel().string() == "Flow build, choose another")
        #expect(try text("task-flow-latest-name") == "Latest: feature")
        #expect(await source.controls.count == 1)
        try find("task-flow-start").button().tap()
        for _ in 0..<20 where (try? find("task-flow-error")) == nil { try await settle(window) }
        #expect(try text("task-flow-error") == "Task flow \"build\" was refused by the fixture")
        #expect(await source.controls.last == ["--task", "W2-131", "flow", "start", "build"])
        #expect(try find("flow-node-4").accessibilityLabel().string() == "demo, stopped here")

        // Execution advances through the shared read; the graph follows it.
        await source.advanceReview()
        await model.refresh()
        try await settle(window)
        // Running reads `● step · elapsed · provider`; the fixture's runtime record
        // dates from July, so only the fixed parts are pinned here.
        let running = try text("task-flow-status")
        #expect(running.hasPrefix("realign · ") && running.hasSuffix(" · claude"), "was \(running)")
        #expect(try find("flow-node-2").accessibilityLabel().string() == "realign, running")
        #expect((try? find("task-flow-pause")) == nil)
        try captureIfRequested(window, name: "task-flow-running")

        // The Session and its companion survived every Flow interaction.
        let record = try #require(model.sessions.value?.first { $0.id == "design" })
        model.navigation.content = .terminals
        model.navigation.selectedSessionId = record.id
        workspace.multiplexer.setFocusedPane(shells[0])
        try await settle(window)
        #expect(workspace.multiplexer.layout == layout)
        for (index, surface) in surfaces.enumerated() {
            #expect(terminals[index].surface == surface)
            let reply = index == 0 ? draft : "companion-responds"
            let input = index == 0 ? "\n" : reply + "\n"
            input.withCString { ghostty_surface_text(surface, $0, UInt(input.utf8.count)) }
            let deadline = ContinuousClock.now + .seconds(3)
            while terminalText(surface).components(separatedBy: reply).count < 3, ContinuousClock.now < deadline {
                try await Task.sleep(for: .milliseconds(20))
            }
            #expect(terminalText(surface).components(separatedBy: reply).count == 3)
        }
    }

    private func press(_ character: String, keyCode: UInt16, modifiers: NSEvent.ModifierFlags = [], in window: NSWindow) throws {
        for type in [NSEvent.EventType.keyDown, .keyUp] {
            let event = try #require(NSEvent.keyEvent(with: type, location: .zero, modifierFlags: modifiers,
                timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber, context: nil,
                characters: character, charactersIgnoringModifiers: character, isARepeat: false, keyCode: keyCode))
            NSApp.sendEvent(event)
        }
    }

    private func ax(_ element: NSObject, _ property: String) -> Any? {
        let key = property == "Focused" ? "isAccessibilityFocused" : "accessibility" + property
        guard element.responds(to: NSSelectorFromString(key)) else { return nil }
        return element.value(forKey: key)
    }

    private func accessible(_ root: Any) -> [NSObject] {
        guard let element = root as? NSObject else { return [] }
        return [element] + ((ax(element, "Children") as? [Any]) ?? []).flatMap { accessible($0) }
    }

    private func focusedLabel(in window: NSWindow) -> String? {
        accessible(window.contentView!).first { ax($0, "Focused") as? Bool == true }
            .flatMap { ax($0, "Label") as? String }
    }

    private func pressElement(_ id: String, in window: NSWindow) throws {
        let element = try #require(accessible(window.contentView!).first { ax($0, "Identifier") as? String == id })
        let action = NSSelectorFromString("accessibilityPerformPress")
        try #require(element.responds(to: action))
        element.perform(action)
    }

    private func settle(_ window: NSWindow) async throws {
        window.contentView?.layoutSubtreeIfNeeded()
        window.layoutIfNeeded()
        // Drive AppKit's dynamic key loop in this unhosted window, alongside layout.
        window.recalculateKeyViewLoop()
        if let content = window.contentView,
           let bitmap = content.bitmapImageRepForCachingDisplay(in: content.bounds) {
            content.cacheDisplay(in: content.bounds, to: bitmap)
        }
        try await Task.sleep(for: .milliseconds(100))
    }

    private func captureIfRequested(_ window: NSWindow, name: String) throws {
        guard let directory = ProcessInfo.processInfo.environment["LOOPFLOW_FLOW_CAPTURE_DIR"],
              let content = window.contentView,
              let bitmap = content.bitmapImageRepForCachingDisplay(in: content.bounds) else { return }
        content.cacheDisplay(in: content.bounds, to: bitmap)
        let png = try #require(bitmap.representation(using: .png, properties: [:]))
        try png.write(to: URL(fileURLWithPath: directory).appendingPathComponent("\(name).png"))
    }

    private func terminalText(_ surface: ghostty_surface_t) -> String {
        var text = ghostty_text_s()
        let selection = ghostty_selection_s(
            top_left: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_TOP_LEFT, x: 0, y: 0),
            bottom_right: ghostty_point_s(tag: GHOSTTY_POINT_SCREEN, coord: GHOSTTY_POINT_COORD_BOTTOM_RIGHT, x: 0, y: 0),
            rectangle: false
        )
        guard ghostty_surface_read_text(surface, selection, &text) else { return "" }
        defer { ghostty_surface_free_text(surface, &text) }
        guard let bytes = text.text else { return "" }
        return String(decoding: Data(bytes: bytes, count: Int(text.text_len)), as: UTF8.self)
    }
}

/// Shared reads plus recorded controls. Only `flow start` counts as a control;
/// anything else unexpected fails loudly.
private actor FlowSource {
    private var roadmap: [String: Any]
    private var catalog: String
    private let session: String
    private(set) var controls: [[String]] = []

    init(session: Data) throws {
        guard let roadmap = try JSONSerialization.jsonObject(with: fixture("roadmap_snapshot.json")) as? [String: Any],
              var entries = try JSONSerialization.jsonObject(with: fixture("flow_catalog.json")) as? [[String: Any]],
              var graph = entries[0]["graph"] as? [String: Any]
        else { throw RegistryQueryError("Flow proof fixtures are malformed") }
        self.roadmap = roadmap
        var build = entries[0]
        graph["name"] = "build"
        build["name"] = "build"
        build["graph"] = graph
        entries.insert(build, at: 1)
        catalog = String(decoding: try JSONSerialization.data(withJSONObject: entries), as: UTF8.self)
        self.session = String(decoding: session, as: UTF8.self)
    }

    func crossingReturns() throws {
        var entries = try #require(JSONSerialization.jsonObject(with: Data(catalog.utf8)) as? [[String: Any]])
        func steps(_ prefix: String) -> [[String: Any]] {
            (0...2).map { index -> [String: Any] in
                let target = prefix.isEmpty ? "implement" : "fix-implement"
                return ["key": (prefix.isEmpty ? 0 : 4) + index, "id": NSNull(),
                 "label": index == 0 ? target : "loop-decide", "kind": "skill",
                 "human": false, "returns_to": index == 0 ? NSNull() : (prefix.isEmpty ? 0 : 4) as Any,
                 "sources": ["feature"], "paths": []]
            }
        }
        func items(_ prefix: String) -> [[String: Any]] {
            [["kind": "group", "id": "\(prefix)outer", "name": "build", "items": [
                ["kind": "group", "id": "\(prefix)inner", "name": "edit", "items": [
                    ["kind": "node", "key": (prefix.isEmpty ? 0 : 4) + 0, "paths": [:]]
                ]],
                ["kind": "group", "id": "\(prefix)empty", "name": "empty", "items": []]
            ]],
             ["kind": "node", "key": (prefix.isEmpty ? 0 : 4) + 1, "paths": [:]],
             ["kind": "node", "key": (prefix.isEmpty ? 0 : 4) + 2, "paths": [:]]]
        }
        var nodes = steps("")
        nodes.append(["key": 3, "id": NSNull(), "label": "xor-route", "kind": "xor",
                      "human": false, "returns_to": NSNull(), "sources": ["feature"],
                      "paths": [["name": "fix", "description": "Repair", "steps": steps("3/fix/")]]])
        var tree = items("")
        tree.append(["kind": "node", "key": 3, "paths": ["fix": items("3/fix/")]])
        entries[0]["graph"] = ["name": "feature", "steps": nodes]
        entries[0]["template"] = ["revision": "crossing-returns", "items": tree]
        catalog = String(decoding: try JSONSerialization.data(withJSONObject: entries), as: UTF8.self)
    }

    func reviseTemplate() throws {
        var entries = try #require(JSONSerialization.jsonObject(with: Data(catalog.utf8)) as? [[String: Any]])
        var template = try #require(entries[0]["template"] as? [String: Any])
        template["revision"] = "changed-feature-definition"
        entries[0]["template"] = template
        catalog = String(decoding: try JSONSerialization.data(withJSONObject: entries), as: UTF8.self)
    }

    func respond(_ args: [String]) throws -> String {
        switch (args.first, args.dropFirst().first) {
        case ("roadmap", _):
            return String(decoding: try JSONSerialization.data(withJSONObject: roadmap), as: UTF8.self)
        case ("wave", "list"): return "[]"
        case ("activity", _): return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
        case ("session", "list"): return #"{"entries":\#(session),"next":null}"#
        case ("flow", "list"): return catalog
        case ("--task", _) where args.dropFirst(2).starts(with: ["flow", "start"]):
            controls.append(args)
            // W2-131 already has a Flow; the fixture refuses a second launch.
            if args[1] == "W2-131" { throw RegistryQueryError("Task flow \"build\" was refused by the fixture") }
            return ""
        default:
            throw RegistryQueryError("Unexpected Flow proof operation: \(args)")
        }
    }

    /// Replace W2-131's Flow with the fixture's running position.
    func advanceReview() {
        let snapshots = (try? JSONSerialization.jsonObject(with: fixture("task_flow.json"))) as? [[String: Any]]
        guard let running = snapshots?[1],
              var waves = roadmap["waves"] as? [[String: Any]],
              var tasks = waves[0]["tasks"] as? [String: Any],
              var items = tasks["items"] as? [[String: Any]],
              let index = items.firstIndex(where: { ($0["task"] as? [String: Any])?["identifier"] as? String == "W2-131" })
        else { return }
        items[index]["flow"] = running
        tasks["items"] = items
        waves[0]["tasks"] = tasks
        roadmap["waves"] = waves
    }
}
#endif
#endif
