#if os(macOS)
import AppKit
import SwiftUI
import Testing
@testable import Loopflow
@testable import LoopflowMac

@MainActor
struct DemoConfiguredCaptureTests {
    @Test func configuredReadAndRender() async throws {
        _ = NSApplication.shared
        let query = RegistryQuery { args, cwd in
            try await Task.detached {
                let process = LocalWaveAgentLauncher.queryProcess(
                    ["/Applications/Loopflow.app/Contents/MacOS/lf"] + args, cwd: cwd)
                let output = Pipe()
                let errors = Pipe()
                process.standardOutput = output
                process.standardError = errors
                try process.run()
                let data = output.fileHandleForReading.readDataToEndOfFile()
                let diagnosis = errors.fileHandleForReading.readDataToEndOfFile()
                process.waitUntilExit()
                guard process.terminationStatus == 0 else {
                    throw RegistryQueryError(String(decoding: diagnosis, as: UTF8.self))
                }
                return String(decoding: data, as: UTF8.self)
            }.value
        }
        let model = PodiumModel(query: query, repoPath: "/Users/jack/src/loopflow")
        await model.refresh()
        #expect(model.roadmap.errorMessage == nil)
        #expect(model.sessions.errorMessage == nil)
        #expect(model.visibleRoadmaps.count == 3)
        #expect(model.visibleRoadmaps.allSatisfy { $0.chapter?.id == "demo-20260924" })
        let task = try #require(model.visibleRoadmaps.flatMap { $0.tasks.items }.first { $0.task.identifier == "LOO-291" })
        model.navigation.presentation = .full
        model.select(.task(id: task.id))
        let view = HStack(spacing: 0) {
            WorkspaceNavigator(model: model, onOpenSession: { _ in }).frame(width: 330)
            WorkSurfaceView(model: model).frame(width: 870)
        }.frame(width: 1200, height: 850)
            .environment(\.colorScheme, .light)
            .environment(\.palette, LoopflowPalette.light)
            .foregroundStyle(LoopflowPalette.light.text)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1200, height: 850),
                              styleMask: [], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: view)
        defer { window.contentView = nil; window.close() }
        try await Task.sleep(for: .milliseconds(300))
        _ = try SnapshotService().snapshotWindow(window, to: "/tmp/loo291-demo-configured-ready.png")
    }
}
#endif
