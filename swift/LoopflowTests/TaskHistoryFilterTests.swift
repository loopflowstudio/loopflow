#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Task history")
@MainActor
struct TaskHistoryFilterTests {
    private let now = ISO8601DateFormatter().date(from: "2026-10-02T12:00:00Z")!
    private var fixtures: URL {
        URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto")
    }

    private func historyRows() throws -> [[String: Any]] {
        try #require(JSONSerialization.jsonObject(
            with: Data(contentsOf: fixtures.appendingPathComponent("task_history_rows.json"))
        ) as? [[String: Any]])
    }

    private func roadmapWire() throws -> [String: Any] {
        var root = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json"))) as? [String: Any])
        var waves = try #require(root["waves"] as? [[String: Any]])
        var evidence = try #require(waves[0]["tasks"] as? [String: Any])
        var rows = try historyRows()
        if let index = rows.firstIndex(where: { ($0["task"] as? [String: Any])?["id"] as? String == "unresolved" }) {
            rows[index]["runtime"] = ["work_id": "task-unresolved", "status": "ready", "reason": "Running", "updated_at": "2026-10-02T12:00:00Z", "provider": "codex", "started": true] as [String: Any]
            var condition = try #require(rows[index]["condition"] as? [String: Any])
            condition["unresolved_execution"] = true
            rows[index]["condition"] = condition
        }
        evidence["items"] = rows
        waves[0]["tasks"] = evidence
        root["waves"] = [waves[0]]
        return root
    }

    private func task(_ state: String, date: String?, runtime: String? = nil, missing: Bool = false, review: Bool = false) throws -> RoadmapTask {
        var row = try #require(historyRows().first)
        var plan = try #require(row["task"] as? [String: Any])
        plan["state"] = state
        plan["completed"] = state == "completed"
        plan["completed_at"] = date.map { $0 as Any } ?? NSNull()
        row["task"] = plan
        if let runtime {
            row["runtime"] = ["work_id": "task", "status": runtime, "reason": "settled", "updated_at": "2026-10-02T12:00:00Z", "provider": "codex", "started": true] as [String: Any]
        }
        row["condition"] = ["state": missing ? "blocked" : "clear", "reason": "historical checkout", "observed_at": "2026-10-02T12:00:00Z", "evidence_age_secs": 0,
            "local_progress": ["state": missing ? "missing" : "not_applicable", "unsettled": missing, "dirty": NSNull(), "authored_commits": NSNull(), "recovery_required": missing, "reason": "checkout removed"],
            "unresolved_execution": runtime == "ready" || review] as [String: Any]
        return try JSONDecoder().decode(RoadmapTask.self, from: JSONSerialization.data(withJSONObject: row))
    }

    private func visible(_ row: RoadmapTask, _ filter: TaskHistoryFilter) -> Bool {
        filter.includes(row.task, condition: row.condition, now: now)
    }

    @Test func rollingBoundariesAndValidation() throws {
        var filter = TaskHistoryFilter()
        filter.showCompleted = true
        for date in ["2026-09-25T12:00:00Z", "2026-09-25T05:00:00-07:00", "2026-10-02T12:00:00.000Z"] {
            #expect(visible(try task("completed", date: date), filter))
        }
        for date in ["2026-09-25T11:59:59.9999Z", "2026-10-02T12:00:00.0001Z", "2026-09-12T12:00:00Z"] {
            #expect(!visible(try task("completed", date: date), filter))
        }
        #expect(!visible(try task("completed", date: nil), filter))
        filter.editDays("30")
        #expect(visible(try task("completed", date: "2026-09-12T12:00:00Z"), filter))
        for invalid in ["", "-1", "1.5", "+1", "999999999999999999999999999"] {
            filter.editDays(invalid)
            #expect(filter.validationMessage != nil)
            #expect(filter.days == 30)
        }
        filter.editDays("0")
        #expect(visible(try task("completed", date: nil), filter))
        for state in ["canceled", "duplicate"] {
            #expect(!visible(try task(state, date: nil), filter))
        }
        #expect(visible(try task("unstarted", date: "2026-01-01T00:00:00Z", runtime: "abandoned"), TaskHistoryFilter()))
        #expect(visible(try task("unknown", date: nil), TaskHistoryFilter()))
    }

    @Test func settledMissingCheckoutDoesNotBypassHistoryWindow() throws {
        let row = try task("completed", date: "2026-09-12T12:00:00Z", runtime: "done", missing: true)
        var filter = TaskHistoryFilter()
        #expect(!visible(row, filter))
        filter.showCompleted = true
        #expect(!visible(row, filter))
        filter.editDays("30")
        #expect(visible(row, filter))
        #expect(visible(try task("canceled", date: nil, runtime: "ready", missing: true), TaskHistoryFilter()))
    }

    @Test func unresolvedReviewAndRetainedSessionSurviveHiddenHistory() throws {
        let row = try task("canceled", date: nil, runtime: "done", missing: true)
        #expect(visible(try task("canceled", date: nil, runtime: "done", missing: true, review: true), TaskHistoryFilter()))
        let sessions = try JSONDecoder().decode([SessionRecord].self, from: Data(contentsOf: fixtures.appendingPathComponent("sessions.json")))
        let retained = WorkspaceTask(id: WorkspaceNodeKey(repo: "/repo", work: .task(id: row.id)), task: row, sessions: [try #require(sessions.first)])
        #expect(!visible(row, TaskHistoryFilter()))
        #expect(retained.inWorkingSet)
        #expect(retained.sessions.first?.id == sessions.first?.id)
    }

    @Test func rollingDurationCrossesDSTWithoutCalendarDayArithmetic() throws {
        var filter = TaskHistoryFilter()
        filter.showCompleted = true
        let boundary = try task("completed", date: "2026-03-05T07:00:00-08:00")
        let before = try task("completed", date: "2026-03-05T06:59:59.9999-08:00")
        let springNow = ISO8601DateFormatter().date(from: "2026-03-12T08:00:00-07:00")!
        #expect(filter.includes(boundary.task, condition: boundary.condition, now: springNow))
        #expect(!filter.includes(before.task, condition: before.condition, now: springNow))
    }

    @Test func inlineEditingAppliesCancelsAndRemembersRange() throws {
        var applied = TaskHistoryFilter()
        let view = TaskHistoryInlineView(filter: applied, onChange: { applied = $0 })
        #expect(view.number.string == "Completed")
        view.activateLabel()
        #expect(applied.showCompleted && applied.days == 7)
        #expect(!view.editing)
        #expect(view.suffix.accessibilityPerformPress())
        #expect(view.editing && view.number.isEditable)
        #expect(view.number.selectedRange() == NSRange(location: 0, length: 1))
        view.number.string = "30"
        #expect(applied.days == 7)
        #expect(view.textView(view.number, doCommandBy: #selector(NSResponder.insertNewline(_:))))
        #expect(applied.days == 30 && !view.editing)
        #expect(view.number.acceptsFirstResponder && view.number.canBecomeKeyView)
        view.activateLabel()
        #expect(view.textView(view.number, doCommandBy: #selector(NSResponder.insertTab(_:))))
        #expect(!view.editing && applied.days == 30)
        view.activateLabel()
        view.number.string = "90"
        #expect(view.textView(view.number, doCommandBy: #selector(NSResponder.cancelOperation(_:))))
        #expect(applied.days == 30 && view.number.string == "30")
        for invalid in ["", "-1", "1.5", "999999999999999999999999999"] {
            view.activateLabel()
            view.number.string = invalid
            #expect(view.number.resignFirstResponder())
            #expect(applied.days == 30 && applied.validationMessage != nil)
        }
        view.activateLabel()
        view.number.string = "0"
        view.finishEditing()
        #expect(applied.days == 0 && view.number.string == "All Tasks")
        view.activateLabel()
        #expect(view.number.string == "0" && !view.suffix.isHidden)
        view.number.string = "14"
        view.checkbox.state = .off
        view.toggleHistory()
        #expect(!applied.showCompleted && applied.days == 14)
        #expect(view.number.string == "Completed")
        view.activateLabel()
        #expect(applied.showCompleted && applied.days == 14)
        #expect(view.number.string == "14")
    }

    private func inkBounds(_ view: NSView) throws -> (ink: CGRect, pixels: CGSize) {
        let bitmap = try #require(view.bitmapImageRepForCachingDisplay(in: view.bounds))
        view.cacheDisplay(in: view.bounds, to: bitmap)
        var bounds = CGRect.null
        for y in 0..<bitmap.pixelsHigh {
            for x in 0..<bitmap.pixelsWide {
                if let color = bitmap.colorAt(x: x, y: y), color.alphaComponent > 0.1 {
                    bounds = bounds.union(CGRect(x: x, y: y, width: 1, height: 1))
                }
            }
        }
        #expect(!bounds.isNull)
        return (bounds, CGSize(width: bitmap.pixelsWide, height: bitmap.pixelsHigh))
    }

    @Test func numberGlyphsSuffixAndCheckboxDoNotMoveWhenEditing() throws {
        for days in ["7", "30", "365", String(Int.max)] {
            var filter = TaskHistoryFilter()
            filter.showCompleted = true
            filter.editDays(days)
            let view = TaskHistoryInlineView(filter: filter, onChange: { _ in })
            view.layoutSubtreeIfNeeded()
            let text = view.number
            let layout = try #require(text.layoutManager)
            let container = try #require(text.textContainer)
            layout.ensureLayout(for: container)
            let baseline = layout.location(forGlyphAt: 0)
            let displayInk = try inkBounds(view.numberViewport.contentView)
            #expect(displayInk.ink.height < displayInk.pixels.height)
            let displayBounds = view.numberViewport.contentView.bounds
            let textBounds = text.bounds
            let textOrigin = text.textContainerOrigin
            let numberFrame = view.numberViewport.frame
            let suffixFrame = view.suffix.frame
            let suffixInk = try inkBounds(view.suffix)
            let checkboxFrame = view.checkbox.frame
            view.activateLabel()
            view.layoutSubtreeIfNeeded()
            #expect(view.number === text)
            #expect(text.selectedRange() == NSRange(location: 0, length: days.count))
            #expect(text.textContainerOrigin == textOrigin)
            #expect(text.bounds == textBounds)
            #expect(view.numberViewport.contentView.bounds == displayBounds)
            // Remove only selection paint; capture the same actual native surface
            // through the same clip view, bounds and bitmap scale in both modes.
            text.setSelectedRange(NSRange(location: 0, length: 0))
            let editingInk = try inkBounds(view.numberViewport.contentView)
            #expect(editingInk.pixels == displayInk.pixels)
            #expect(editingInk.ink == displayInk.ink)
            layout.ensureLayout(for: container)
            #expect(layout.location(forGlyphAt: 0) == baseline)
            #expect(view.numberViewport.frame == numberFrame)
            #expect(view.suffix.frame == suffixFrame)
            #expect(try inkBounds(view.suffix).ink == suffixInk.ink)
            #expect(view.checkbox.frame == checkboxFrame)
            #expect(view.suffix.frame.maxX <= view.bounds.maxX)
            view.finishEditing()
            view.layoutSubtreeIfNeeded()
            #expect(view.numberViewport.frame == numberFrame && view.suffix.frame == suffixFrame)
        }
    }

    @Test func waveControlsFilterRowsAndCountTogether() async throws {
        let wire = try roadmapWire()
        let payload = String(decoding: try JSONSerialization.data(withJSONObject: wire), as: UTF8.self)
        let catalog = try String(contentsOf: fixtures.appendingPathComponent("flow_catalog.json"), encoding: .utf8)
        let query = RegistryQuery { args, _ in args == ["flow", "list", "--json"] ? catalog : payload }
        let roadmap = try await query.roadmap()
        let wave = try #require(roadmap.waves.first)
        let model = PodiumModel(query: query)
        model.applyFixture(roadmap: .available(roadmap), waves: .available([wave.wave.toWave()]), processActivity: .loading, workActivity: .loading, repos: [])
        await model.loadFlowCatalog()
        model.select(.wave(id: wave.wave.id))
        model.taskHistoryNow = now
        var opened: WorkReference?
        let view = WorkSurfaceView(model: model, onOpenTask: { opened = $0 })
        func rows() throws -> [String] {
            try view.inspect().findAll(ViewType.Button.self).compactMap { button in
                guard let id = try? button.accessibilityIdentifier(), id.hasPrefix("podium-task-") else { return nil }
                return String(id.dropFirst("podium-task-".count))
            }
        }
        func checkCount() throws {
            let visibleRows = try rows()
            for id in [318, 315, 314, 313, 310, 308, 307] {
                #expect(!visibleRows.contains("LOO-\(id)"))
            }
            #expect(visibleRows.contains("unresolved"))
            for id in visibleRows {
                let button = try view.inspect().find(viewWithAccessibilityIdentifier: "podium-task-\(id)").button()
                #expect(button.isDisabled() == false)
                try button.tap()
                #expect(opened == .task(id: id))
            }
            let heading = try view.inspect().find(viewWithAccessibilityIdentifier: "wave-task-count")
            let count = try heading.findAll(ViewType.Text.self).map { try $0.string() }
            #expect(count.contains(String(visibleRows.count)))
        }
        try checkCount()
        _ = try view.inspect().find(text: "Canceled")
        #expect(try rows() == ["LOO-309", "LOO-316", "LOO-312", "LOO-306", "reopened", "unresolved"])
        let control = try view.inspect().find(TaskHistoryInlineControl.self).actualView()
        let native = TaskHistoryInlineView(filter: control.filter, onChange: { control.filter = $0 })
        native.activateLabel()
        model.taskHistoryNow = now
        #expect(try rows().contains("recent"))
        try checkCount()
        #expect(try !rows().contains("old"))
        native.activateLabel()
        native.number.string = "30"
        native.finishEditing()
        model.taskHistoryNow = now
        #expect(try rows().contains("old"))
        try checkCount()
        native.activateLabel()
        native.number.string = "-1"
        native.finishEditing()
        #expect(model.taskHistoryFilters[wave.wave.id]?.days == 30)
        try checkCount()
        _ = try view.inspect().find(text: "Enter a whole number of 0 or more. Previous range kept.")
        native.activateLabel()
        native.number.string = "0"
        native.finishEditing()
        #expect(try rows().contains("unknown"))
        try checkCount()
        _ = try view.inspect().find(text: "Completed · date unavailable")
        #expect(model.taskHistoryFilters[wave.wave.id]?.showCompleted == true)
        // Inspection remains available even for hidden terminal inventory;
        // Rust's admission decision still governs the production Start button.
        for id in ["LOO-318", "recent", "LOO-309"] {
            model.select(.task(id: id))
            let start = try view.inspect().find(viewWithAccessibilityIdentifier: "task-flow-start").button()
            #expect(start.isDisabled() == (id != "LOO-309"))
        }
    }
}
#endif
