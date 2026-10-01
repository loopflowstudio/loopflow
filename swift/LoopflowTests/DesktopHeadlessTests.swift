#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

// Inspect production view bodies and invoke controls directly. No NSApplication,
// NSWindow, window server connection, screenshot capture, or UI automation.
@Suite("Desktop without a display")
@MainActor
struct DesktopHeadlessTests {
    @Test("Task work renders checkout conversations, the managed Flow and mechanical Execs")
    func taskWorkInventory() async throws {
        let fixtures = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto")
        let data = try Data(contentsOf: fixtures.appendingPathComponent("task_work.json"))
        let work = try JSONDecoder().decode(TaskWork.self, from: data)
        let payload = "{\"work\":\(String(decoding: data, as: UTF8.self))}"
        let query = RegistryQuery { _, _ in payload }
        let model = PodiumModel(query: query)
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self,
            from: Data(contentsOf: fixtures.appendingPathComponent("roadmap_snapshot.json")))
        let wave = try #require(roadmap.waves.first)
        let task = try #require(wave.tasks.items.first)
        await model.loadTaskWork(task: task, wave: wave.wave)
        let view = TaskWorkView(model: model, task: task, wave: wave.wave)
        for id in work.sessions.map(\.id) + work.flows.map(\.id) + work.execs.map(\.id) {
            _ = try view.inspect().find(viewWithAccessibilityIdentifier: "task-work-\(id)")
        }
        #expect(try view.inspect().findAll(ViewType.Text.self) {
            try $0.string() == "Managed"
        }.count == 2)
    }

    @Test("Loading, unavailable, empty and selected Work have distinct content")
    func workStates() throws {
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Unexpected external read") }
        let model = PodiumModel(query: query)
        let view = WorkSurfaceView(model: model)
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "podium-work-loading")

        model.applyFixture(roadmap: .unavailable(lastGood: nil, reason: "offline"),
                           waves: .available([]), processActivity: .loading,
                           workActivity: .loading, repos: [])
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "podium-work-unavailable")
        #expect(throws: (any Error).self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "podium-work-empty")
        }

        let fixture = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")
        let data = try Data(contentsOf: fixture)
        var wire = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        wire["waves"] = []
        let empty = try JSONDecoder().decode(RoadmapSnapshot.self, from: JSONSerialization.data(withJSONObject: wire))
        model.applyFixture(roadmap: .available(empty), waves: .available([]),
                           processActivity: .loading, workActivity: .loading, repos: [])
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "podium-work-empty")

        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: data)
        let wave = try #require(roadmap.waves.first).wave
        model.applyFixture(roadmap: .available(roadmap), waves: .available([wave.toWave()]),
                           processActivity: .loading, workActivity: .loading, repos: [])
        model.navigation.presentation = .full
        let navigator = WorkspaceNavigator(model: model, onOpenSession: { _ in })
        try navigator.inspect()
            .find(viewWithAccessibilityIdentifier: "workspace-wave-\(wave.id)").button().tap()

        #expect(model.selection == .wave(id: wave.id))
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "podium-detail-wave")
        let title = try view.inspect().find(viewWithAccessibilityIdentifier: "wave-title").text().string()
        #expect(title == "Product")
        #expect(throws: (any Error).self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "podium-work-loading")
        }
    }
}
#endif
