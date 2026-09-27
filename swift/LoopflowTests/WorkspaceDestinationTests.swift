#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Workspace destinations", .serialized)
@MainActor
struct WorkspaceDestinationTests {
    @Test func taskLinkParsesOneDecodedIdentifier() throws {
        let link = try TaskLink(url: #require(URL(string: "loopflow://task/LOO-303?repo=%2Fsrc%2Fspace%20here")))
        #expect(link.issue == "LOO-303")
        #expect(link.repo == "/src/space here")
        for value in ["loopflow://task/", "loopflow://task/A/B", "loopflow://task/A%2FB", "loopflow://task/A%0AB", "loopflow://task/A?repo=", "loopflow://task/A?repo=x&repo=y", "loopflow://task/A#node"] {
            let url = try #require(URL(string: value))
            #expect(throws: (any Error).self) { try TaskLink(url: url) }
        }
    }

    @Test func historicalTaskSurvivesCurrentPlanRefresh() async throws {
        let data = try fixture()
        let query = RegistryQuery { args, _ in
            if args.first == "roadmap" {
                return args.contains("--task") ? data : #"{"generated_at":"2026-09-26T00:00:00Z","waves":[]}"#
            }
            if args.first == "session" || args.first == "ls" { return "[]" }
            throw RegistryQueryError("No mutation permitted in navigation proof")
        }
        let model = PodiumModel(query: query)
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        // Exercise the production URL handler with one exact shared-reader result.
        let exact = try oneTask(data, taskId: task.id)
        let exactQuery = RegistryQuery { args, _ in
            if args.contains("--task") { return exact }
            if args.first == "roadmap" { return #"{"generated_at":"2026-09-26T00:00:00Z","waves":[]}"# }
            if args.first == "session" || args.first == "ls" { return "[]" }
            throw RegistryQueryError("No mutation permitted")
        }
        let destination = PodiumModel(query: exactQuery)
        await destination.openTaskLink(try #require(URL(string: "loopflow://task/\(task.task.identifier)")))
        #expect(destination.selection == .task(id: task.id))
        #expect(destination.navigation.content == .details)
        #expect(destination.navigation.selectedSessionId == nil)
        await destination.refresh()
        #expect(destination.selection == .task(id: task.id))
        #expect(destination.task(id: task.id)?.task.task.name == task.task.name)
        #expect(destination.visibleRoadmaps.isEmpty)
        #expect(destination.breadcrumb?.task?.task.id == task.id)
        #expect(throws: Never.self) {
            try WorkSurfaceView(model: destination).inspect().find(viewWithAccessibilityIdentifier: "podium-detail-task")
        }
        await model.refresh()
        #expect(model.selection == nil)
    }

    @Test func paletteSearchIncludesUnstartedTasksAndRanksExactIDs() async throws {
        let data = try fixture()
        let model = PodiumModel(query: RegistryQuery { args, _ in
            if args.first == "roadmap" { return data }
            if args.first == "ls" || args.first == "session" { return "[]" }
            throw RegistryQueryError("Unavailable")
        }, repoPath: "/src/loopflow")
        await model.refresh()
        let tasks = model.visibleRoadmaps.flatMap { $0.tasks.items }
        let task = try #require(tasks.first)
        #expect(model.searchDestinations(task.task.identifier).first?.id == .task(task.id))
        #expect(model.paletteRows.filter { if case .task = $0.id { true } else { false } }.count == tasks.count)
        model.remember(.task(task.id))
        #expect(model.searchDestinations("").first?.id == .task(task.id))
        for index in 0..<30 { model.remember(.session("\(index)")) }
        #expect(model.navigation.recentDestinations.count == 20)
    }

    @Test func lateLinkCannotReplaceLaterSelection() async throws {
        let data = try fixture()
        let snapshot = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(data.utf8))
        let wave = try #require(snapshot.waves.first)
        let task = try #require(wave.tasks.items.first)
        let exact = try oneTask(data, taskId: task.id)
        let barrier = DestinationReadBarrier()
        let model = PodiumModel(query: RegistryQuery { args, _ in
            if args.contains("--task") { await barrier.wait(); return exact }
            if args.first == "roadmap" { return data }
            if args.first == "ls" || args.first == "session" { return "[]" }
            throw RegistryQueryError("Unavailable")
        }, repoPath: wave.wave.repo)
        await model.refresh()
        let read = Task { await model.openTaskLink(URL(string: "loopflow://task/A-1")!) }
        while !(await barrier.started) { await Task.yield() }
        model.select(.wave(id: wave.wave.id))
        await barrier.release()
        await read.value
        #expect(model.selection == .wave(id: wave.wave.id))
        #expect(!model.showsTaskLink)
    }

    @Test func ambiguousAndUnavailableLinksPreserveTheWorkspace() async throws {
        let data = try fixture()
        for response in [data, #"{"generated_at":"2026-09-26T00:00:00Z","waves":[]}"#, "transport-error"] {
            let model = PodiumModel(query: RegistryQuery { args, _ in
                if args.contains("--task") {
                    if response == "transport-error" { throw RegistryQueryError("Home unavailable") }
                    return response
                }
                if args.first == "roadmap" { return data }
                if args.first == "ls" || args.first == "session" { return "[]" }
                throw RegistryQueryError("No mutation permitted")
            }, repoPath: "/src/loopflow")
            await model.refresh()
            let wave = try #require(model.visibleRoadmaps.first)
            model.select(.wave(id: wave.wave.id))
            await model.openTaskLink(try #require(URL(string: "loopflow://task/LOO-303")))
            #expect(model.selection == .wave(id: wave.wave.id))
            #expect(model.showsTaskLink)
            if response == "transport-error" {
                #expect(model.taskLinkReading.errorMessage == "Home unavailable")
            } else {
                #expect(model.taskLinkReading.errorMessage == nil)
            }
        }
    }

    @Test func coldAndWarmLinksReachOnlyOneWorkspace() throws {
        _ = NSApplication.shared
        let router = WorkspaceLinkRouter()
        let url = try #require(URL(string: "loopflow://task/LOO-303"))
        #expect(!router.deliver(url))
        let window = NSWindow(contentRect: .init(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        let other = NSWindow(contentRect: .init(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        defer { window.orderOut(nil); other.orderOut(nil) }
        var received: [URL] = []
        var otherReceived: [URL] = []
        router.register(UUID(), window: window) { received.append($0) }
        router.register(UUID(), window: other) { otherReceived.append($0) }
        #expect(received == [url])
        #expect(otherReceived.isEmpty)
        window.makeKeyAndOrderFront(nil)
        #expect(router.deliver(url))
        #expect(received == [url, url])
        #expect(otherReceived.isEmpty)
    }

    private func fixture() throws -> String {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        return try String(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json"), encoding: .utf8)
    }

    private func oneTask(_ data: String, taskId: String) throws -> String {
        var snapshot = try #require(JSONSerialization.jsonObject(with: Data(data.utf8)) as? [String: Any])
        var waves = try #require(snapshot["waves"] as? [[String: Any]])
        for index in waves.indices {
            var evidence = try #require(waves[index]["tasks"] as? [String: Any])
            let items = evidence["items"] as? [[String: Any]] ?? []
            evidence["state"] = "ok"
            evidence["truncated"] = false
            evidence["items"] = items.filter { ($0["task"] as? [String: Any])?["id"] as? String == taskId }
            waves[index]["tasks"] = evidence
        }
        snapshot["waves"] = waves
        return String(decoding: try JSONSerialization.data(withJSONObject: snapshot), as: UTF8.self)
    }
}

private actor DestinationReadBarrier {
    var started = false
    private var continuation: CheckedContinuation<Void, Never>?
    func wait() async {
        started = true
        await withCheckedContinuation { continuation = $0 }
    }
    func release() { continuation?.resume(); continuation = nil }
}
#endif
