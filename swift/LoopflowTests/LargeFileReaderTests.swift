#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
@testable import Loopflow
@testable import LoopflowMac

/// Reading a large source file in Files: opening it, ⌘F, a click and a scroll,
/// each timed as the main-thread work it causes, including layout.
@Suite("Large file reader", .serialized)
@MainActor
struct LargeFileReaderTests {
    private nonisolated static let path = "swift/LoopflowMac/WorkModel.swift"

    @Test func findClickAndScrollStayResponsiveInALargeSourceFile() async throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let text = try String(contentsOf: root.appendingPathComponent(Self.path), encoding: .utf8)
        let store = try Self.store(text)
        store.selectFile(Self.path)
        await store.document(Self.path).reconcile(Self.snapshot(text))

        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 1000, height: 800),
                              styleMask: [.titled, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.contentView = nil; window.close() }
        let opening = DispatchTime.now().uptimeNanoseconds
        // Unfocused, so Find never takes first responder: AppKit's one-time-code
        // autofill for a focused field exits a process denied the WindowServer.
        window.contentView = NSHostingView(rootView: TaskFilesView(store: store, checkoutBase: nil, isFocused: false))
        var scroll: NSScrollView?
        var openFrame = 0.0
        for _ in 0..<500 {
            openFrame = max(openFrame, window.measureFrame {})
            scroll = window.contentView?.tallestScrollView
            if (scroll?.documentView?.frame.height ?? 0) > 10_000 { break }
            try await Task.sleep(for: .milliseconds(10))
        }
        let opened = Double(DispatchTime.now().uptimeNanoseconds - opening) / 1_000_000
        let reader = try #require(scroll)
        let idle = (0..<5).map { _ in window.measureFrame {} }.max() ?? 0

        let find = FileFind()
        let opens = window.measureFrame {
            store.find = find
            find.query = "store"
        }
        for _ in 0..<500 where find.searching {
            _ = window.measureFrame {}
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(find.matches.count > 20)
        // The last match sits near the end of the file, far below the first.
        let steps = window.measureFrame { find.step(-1) }
        let stepped = try #require(find.current)
        guard case .line(let farLine) = stepped.cell else { Issue.record("Expected a source line"); return }
        #expect(farLine > 3000)

        let clicks = window.measureFrame {
            store.jumps.target = .init(path: Self.path, line: farLine, word: "store", range: stepped.range)
        }
        var scrolls: [Double] = []
        let top = reader.contentView.bounds.minY
        for step in 0..<60 {
            scrolls.append(window.measureFrame {
                reader.contentView.scroll(to: CGPoint(x: 0, y: max(0, reader.contentView.bounds.minY - 40)))
                reader.reflectScrolledClipView(reader.contentView)
                // The reader names the first visible line while a person scrolls.
                store.jumps.readingLine = max(1, farLine - 3 * step)
            })
        }
        let report = """
        \(text.split(separator: "\n", omittingEmptySubsequences: false).count) lines: open \(Self.ms(opened)) ms (wall, longest frame \(Self.ms(openFrame)) ms), idle frame \(Self.ms(idle)) ms, \
        find \(Self.ms(opens)) ms, step to line \(farLine) \(Self.ms(steps)) ms, click \(Self.ms(clicks)) ms, \
        scroll median \(Self.ms(scrolls.sorted()[scrolls.count / 2])) max \(Self.ms(scrolls.max() ?? 0)) ms
        """
        print(report)
        #expect(reader.contentView.bounds.minY < top)
        // Every built row repainting for each moved line cost 126 ms a frame on this file;
        // repainting only changed lines costs about 30 ms on a loaded Machine.
        #expect(scrolls.sorted()[scrolls.count / 2] < 90, "\(report)")
    }

    private static func snapshot(_ text: String) -> TaskFileSnapshot {
        TaskFileSnapshot(recoveries: [], issueIdentifier: "TEST-1", taskId: "task-files", path: path, content: text,
                         data: nil, state: .text, revision: "disk", readOnlyReason: nil, sizeBytes: UInt64(text.utf8.count))
    }

    /// A checkout with no changes whose only readable file is `text`.
    private static func store(_ text: String) throws -> TaskFilesStore {
        let fixture = try Data(contentsOf: URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto/task_files.json"))
        let store = TaskFilesStore(issue: "TEST-1", cwd: "/fixture", query: RegistryQuery { args, _ in
            let root = try JSONSerialization.jsonObject(with: fixture) as! [String: Any]
            guard var value = root[args[1]] as? [String: Any] else { throw RegistryQueryError("Unexpected read") }
            switch args[1] {
            case "changes": value["files"] = []
            case "file":
                guard args[3] == path else { throw RegistryQueryError("no such file") }
                value.merge(["path": path, "content": text, "revision": "disk", "size_bytes": text.utf8.count, "recoveries": []]) { $1 }
            default: break
            }
            return String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
        })
        store.autosave = false
        return store
    }

    private static func ms(_ value: Double) -> String { String(format: "%.1f", value) }
}
#endif

