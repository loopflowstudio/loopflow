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
    @Test("Skill selection persists per repository and only New Session launches")
    func selectedSkill() throws {
        let defaults = UserDefaults.standard
        let key = "sessionSkillsByRepository"
        let before = defaults.object(forKey: key)
        defer { defaults.set(before, forKey: key) }
        let repo = "/tmp/session-picker-" + UUID().uuidString
        let other = repo + "-other"
        let query = RegistryQuery { _, _ in throw RegistryQueryError("Unexpected read") }
        let model = PodiumModel(query: query, repoPath: repo)
        #expect(model.selectedSessionSkill == "capture-tasks")
        var launches: [SessionSkillLaunch] = []
        let view = WorkspaceNavigator(model: model, onOpenSession: { _ in }, onNewSession: { launches.append($0) })
        try view.inspect().find(viewWithAccessibilityIdentifier: "workspace-session-skill").button().tap()
        #expect(launches.isEmpty)
        model.selectSessionSkill("wave/session", repo: repo)
        #expect(launches.isEmpty)
        #expect(model.selectedSessionSkill == "wave/session")
        let updated = WorkspaceNavigator(model: model, onOpenSession: { _ in }, onNewSession: { launches.append($0) })
        try updated.inspect().find(viewWithAccessibilityIdentifier: "workspace-create-task").button().tap()
        #expect(launches.map { $0.arguments(lf: "lf") } == [["lf", "--mode", "interactive", "skill", "wave/session"]])
        #expect(PodiumModel(query: query, repoPath: repo).selectedSessionSkill == "wave/session")
        #expect(PodiumModel(query: query, repoPath: other).selectedSessionSkill == "capture-tasks")
        model.selectSessionSkill("debug", repo: other)
        #expect(model.selectedSessionSkill == "wave/session")
        #expect(PodiumModel(query: query, repoPath: other).selectedSessionSkill == "debug")
    }

    @Test("Skill discovery includes namespaced skills and keeps failures explicit")
    func discovery() async throws {
        let json = """
        [
          {"name":"design","kind":"flow","source":"builtin","description":"Flow","invocation":"lf flow design"},
          {"name":"wave/session","kind":"skill","source":"builtin","description":"Talk about a Wave","invocation":"lf skill wave/session"},
          {"name":"design","kind":"skill","source":".lf/skills/design.md","description":"Local design","invocation":"lf skill design"}
        ]
        """
        let query = RegistryQuery { _, cwd in
            guard cwd == "/tmp/selected-repo" else { throw RegistryQueryError("Repository unavailable") }
            return json
        }
        let skills = try await query.sessionSkills(cwd: "/tmp/selected-repo")
        #expect(skills.map(\.name) == ["design", "wave/session"])
        #expect(skills.first?.description == "Local design")
        await #expect(throws: RegistryQueryError.self) { try await query.sessionSkills(cwd: "/missing") }
        let encoded = try JSONEncoder().encode(skills)
        #expect(try JSONDecoder().decode([DiscoveryEntry].self, from: encoded) == skills)
    }

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
            var captured: SessionSkillLaunch?
            let view = WorkspaceNavigator(model: model, onOpenSession: { _ in }, onNewSession: { captured = $0 })
            try view.inspect().find(viewWithAccessibilityIdentifier: "workspace-create-task").button().tap()
            let launch = try #require(captured)
            let name = selection == nil || selection?.id == "missing" ? nil : wave.wave.name
            #expect(launch == SessionSkillLaunch(repoPath: "/src/loopflow", wave: name, skill: "capture-tasks"))
            let expected = ["/path with spaces/lf", "--mode", "interactive"] + (name.map { ["--wave", $0] } ?? []) + ["skill", "capture-tasks"]
            #expect(launch.arguments(lf: "/path with spaces/lf") == expected)
            #expect(model.selection == selection)
        }
        model.select(.task(id: task.id))
        model.navigation.content = .overview
        #expect(model.sessionSkillLaunch?.wave == nil)
    }

    @Test("Explicit Wave capture retains its own destination")
    func explicitWave() throws {
        let model = try makeModel()
        let wave = try #require(model.visibleRoadmaps.first)
        let task = try #require(wave.tasks.items.first)
        model.select(.task(id: task.id))
        let launch = model.sessionSkillLaunch(for: .wave(id: wave.wave.id))
        #expect(launch == SessionSkillLaunch(repoPath: "/src/loopflow", wave: wave.wave.name, skill: "capture-tasks"))
        #expect(model.selection == .task(id: task.id))
    }

    @Test("No repository means no capture entry point")
    func noRepository() throws {
        let model = PodiumModel(query: RegistryQuery { _, _ in throw RegistryQueryError("Unexpected read") })
        #expect(model.sessionSkillLaunch == nil)
        let view = WorkspaceNavigator(model: model, onOpenSession: { _ in }, onNewSession: { _ in })
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
        let homeId = "capture-test-home"
        let repoIdentity = WorkspaceIdentity(homeId: homeId, worktree: repo)
        let taskIdentity = WorkspaceIdentity(homeId: homeId, worktree: taskPath)
        let registry = SessionsWorkspaceRegistry(localHomeId: homeId)
        let repoPanes = registry.workspace(for: repoIdentity).multiplexer
        repoPanes.newShell(command: ["retained-shell"])
        repoPanes.load(sessionId: "existing-conversation")
        let retained = repoPanes.layout.allPanes
        let taskPanes = registry.workspace(for: taskIdentity).multiplexer
        taskPanes.newShell(command: ["task-shell"])
        let taskLayout = taskPanes.layout
        registry.layout(for: repoIdentity).select(taskIdentity)
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
        #expect(registry.layout(for: repoIdentity).focusedPath == taskIdentity)

        let helper = URL(fileURLWithPath: host).appendingPathComponent("lf")
        try fm.setAttributes([.posixPermissions: 0o755], ofItemAtPath: helper.path)
        try view.inspect().find(viewWithAccessibilityIdentifier: "workspace-create-task").button().tap()
        #expect(model.selection == .task(id: task.id))
        #expect(model.navigation.showsRetainedTerminals)
        #expect(model.navigation.content == .terminals)
        #expect(registry.layout(for: repoIdentity).focusedPath == repoIdentity)
        #expect(taskPanes.layout == taskLayout)
        for pane in retained { #expect(repoPanes.layout.pane(for: pane.id) == pane) }
        #expect(repoPanes.shellCommands[repoPanes.focusedPaneId] ==
            SessionSkillLaunch(repoPath: repo, wave: wave.wave.name, skill: "capture-tasks").arguments(lf: helper.path))
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
