#if os(macOS)
import AppKit
import SwiftUI
import Testing
@testable import Loopflow
@testable import LoopflowMac

@MainActor
struct DemoVisualCaptureTests {
    @Test func capture() async throws {
        _ = NSApplication.shared
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let data = try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json"))
        let object = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let rows = try #require(object["waves"] as? [[String: Any]])
        let waveObjects = try rows.map { try #require($0["wave"] as? [String: Any]) }
        let waves = String(decoding: try JSONSerialization.data(withJSONObject: waveObjects), as: UTF8.self)
        let json = String(decoding: data, as: UTF8.self)
        let query = RegistryQuery { args, _ in
            switch args.first {
            case "roadmap": return json
            case "ls": return waves
            case "session": return "[]"
            case "activity": return #"{"generated_at":1,"since":0,"limit":50,"truncated":false,"items":[]}"#
            default: throw RegistryQueryError("unavailable")
            }
        }
        let model = PodiumModel(query: query, repoPath: "/src/loopflow")
        await model.refresh()
        model.navigation.presentation = .full
        model.select(.task(id: "issue-review"))
        let view = HStack(spacing: 0) {
            WorkspaceNavigator(model: model, onOpenSession: { _ in }).frame(width: 320)
            WorkSurfaceView(model: model).frame(width: 720)
        }.frame(width: 1040, height: 720)
            .environment(\.colorScheme, .light)
            .environment(\.palette, LoopflowPalette.light)
            .foregroundStyle(LoopflowPalette.light.text)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1040, height: 720),
                              styleMask: [], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: view)
        defer { window.contentView = nil; window.close() }
        try await Task.sleep(for: .milliseconds(300))
        _ = try SnapshotService().snapshotWindow(window, to: "/tmp/loo291-demo-style.png")
    }
}
#endif
