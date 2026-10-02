#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

private final class CaptureBundleMarker: NSObject {}

@Suite("TaskCapture without a display", .serialized)
@MainActor
struct TaskCaptureTests {
    @Test("Capture button uses repository or parent Wave without moving selection")
    func captureScopes() throws {
        let model = try makeModel()
        let wave = try #require(model.visibleRoadmaps.first)
        let task = try #require(wave.tasks.items.first)
        let selections: [WorkReference?] = [nil, .wave(id: wave.wave.id), .task(id: task.id), .wave(id: "missing")]
        for selection in selections {
            model.select(selection)
            if selection?.id == "missing" {
                model.navigation.selection = selection
                model.navigation.content = .details
            }
            var captured: TaskCaptureLaunch?
            let view = WorkspaceNavigator(model: model, onOpenSession: { _ in }, onCaptureTask: { captured = $0 })
            try view.inspect().find(viewWithAccessibilityIdentifier: "workspace-create-task").button().tap()
            let launch = try #require(captured)
            let name = selection == nil || selection?.id == "missing" ? nil : wave.wave.name
            #expect(launch == TaskCaptureLaunch(repoPath: "/src/loopflow", wave: name))
            let expected = ["/path with spaces/lf", "--mode", "interactive"] + (name.map { ["--wave", $0] } ?? []) + ["capture-tasks"]
            #expect(launch.arguments(lf: "/path with spaces/lf") == expected)
            #expect(model.selection == selection)
        }
        model.select(.task(id: task.id))
        model.navigation.content = .overview
        #expect(model.taskCaptureLaunch?.wave == nil)
    }

    @Test("Explicit Wave capture retains its own destination")
    func explicitWave() throws {
        let model = try makeModel()
        let wave = try #require(model.visibleRoadmaps.first)
        let task = try #require(wave.tasks.items.first)
        model.select(.task(id: task.id))
        let launch = model.taskCaptureLaunch(wave: .wave(id: wave.wave.id))
        #expect(launch == TaskCaptureLaunch(repoPath: "/src/loopflow", wave: wave.wave.name))
        #expect(model.selection == .task(id: task.id))
    }

    @Test("No repository means no capture entry point")
    func noRepository() throws {
        let model = PodiumModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Unexpected read") })
        #expect(model.taskCaptureLaunch == nil)
        let view = WorkspaceNavigator(model: model, onOpenSession: { _ in }, onCaptureTask: { _ in })
        #expect(throws: (any Error).self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "workspace-create-task")
        }
    }

    @Test("Production capture preserves both checkout layouts and resolves lf before mutation")
    func productionLaunch() throws {
        // Re-enter in an owned host to exercise real bundle helper resolution
        // without editing SwiftPM's SDK helper or starting a provider.
        let fm = FileManager.default
        guard let host = ProcessInfo.processInfo.environment["LOOPFLOW_CAPTURE_TEST_HOST"] else {
            let directory = fm.temporaryDirectory.appendingPathComponent(UUID().uuidString)
            try fm.createDirectory(at: directory, withIntermediateDirectories: true)
            defer { try? fm.removeItem(at: directory) }
            let source = try #require(Bundle.main.executableURL)
            let executable = directory.appendingPathComponent(source.lastPathComponent)
            try fm.copyItem(at: source, to: executable)
            let helper = directory.appendingPathComponent("lf")
            try "#!/bin/sh\nexit 0\n".write(to: helper, atomically: true, encoding: .utf8)
            try JSONSerialization.data(withJSONObject: ["lf_path": helper.path])
                .write(to: directory.appendingPathComponent("LoopflowDevControl.json"))
            let process = Process()
            process.executableURL = executable
            process.arguments = ["--test-bundle-path", try #require(Bundle(for: CaptureBundleMarker.self).executablePath),
                                 "--testing-library", "swift-testing", "--filter", "TaskCaptureTests/productionLaunch", "--no-parallel"]
            process.environment = ProcessInfo.processInfo.environment.merging(
                ["LOOPFLOW_CAPTURE_TEST_HOST": directory.path], uniquingKeysWith: { _, new in new })
            let output = Pipe()
            process.standardOutput = output
            process.standardError = output
            try process.run()
            let log = output.fileHandleForReading.readDataToEndOfFile()
            process.waitUntilExit()
            print(String(decoding: log, as: UTF8.self))
            #expect(process.terminationStatus == 0)
            #expect(fm.fileExists(atPath: directory.appendingPathComponent("complete").path))
            return
        }
        let model = try makeModel()
        let wave = try #require(model.visibleRoadmaps.first { !$0.tasks.items.isEmpty })
        let task = try #require(wave.tasks.items.first)
        model.select(.task(id: task.id))
        let repo = "/src/loopflow"
        let taskPath = task.reference.workspace?.worktree ?? "/src/task-checkout"
        let registry = SessionsWorkspaceRegistry()
        let repoPanes = registry.workspace(for: repo).multiplexer
        repoPanes.newShell(command: ["retained-shell"])
        repoPanes.load(sessionId: "existing-conversation")
        let retained = repoPanes.layout.allPanes
        let taskPanes = registry.workspace(for: taskPath).multiplexer
        taskPanes.newShell(command: ["task-shell"])
        let taskLayout = taskPanes.layout
        registry.layout(for: repo).select(taskPath)
        let view = SessionsView(model: model, repoPath: repo, workspaces: registry,
            query: RegistryQuery { _, _ in throw RegistryQueryError("Unexpected read") })
        let before = repoPanes.layout
        let content = model.navigation.content
        // Missing executable: invoking the actual button must preserve all state.
        try view.inspect().find(viewWithAccessibilityIdentifier: "workspace-create-task").button().tap()
        #expect(repoPanes.layout == before)
        #expect(taskPanes.layout == taskLayout)
        #expect(model.navigation.content == content)
        #expect(!model.navigation.showsRetainedTerminals)
        #expect(registry.layout(for: repo).focusedPath == taskPath)

        let helper = URL(fileURLWithPath: host).appendingPathComponent("lf")
        try fm.setAttributes([.posixPermissions: 0o755], ofItemAtPath: helper.path)
        try view.inspect().find(viewWithAccessibilityIdentifier: "workspace-create-task").button().tap()
        #expect(model.selection == .task(id: task.id))
        #expect(model.navigation.showsRetainedTerminals)
        #expect(model.navigation.content == .terminals)
        #expect(registry.layout(for: repo).focusedPath == repo)
        #expect(taskPanes.layout == taskLayout)
        for pane in retained { #expect(repoPanes.layout.pane(for: pane.id) == pane) }
        #expect(repoPanes.shellCommands[repoPanes.focusedPaneId] ==
            TaskCaptureLaunch(repoPath: repo, wave: wave.wave.name).arguments(lf: helper.path))
        _ = try view.inspect().find(viewWithAccessibilityIdentifier: "worktree-chip")
        model.select(.task(id: task.id))
        #expect(!model.navigation.showsRetainedTerminals)
        try Data().write(to: URL(fileURLWithPath: host).appendingPathComponent("complete"))
    }

    private func makeModel() throws -> PodiumModel {
        let fixture = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(contentsOf: fixture))
        let model = PodiumModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Unexpected read") }, repoPath: "/src/loopflow")
        model.applyFixture(roadmap: .available(roadmap), waves: .available(roadmap.waves.map { $0.wave.toWave() }),
                           processActivity: .loading, workActivity: .loading, repos: [])
        return model
    }
}
#endif
