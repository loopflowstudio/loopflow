#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
@testable import Loopflow
@testable import LoopflowMac

@MainActor
@Suite("Task Watch payload window")
struct TaskWatchWindowTests {
    @Test("Payload limits include tool input and invisible evidence; evicted calls stay explicit")
    func boundedPayload() async throws {
        let store = TaskWatchStore()
        let empty = try page([])
        await store.readOutput(issue: "LOO-293", query: RegistryQuery { _, _ in empty })
        let input = String(repeating: "x", count: 6 * 1_024 * 1_024)
        for id in ["a", "b", "c"] {
            let data = try page([tool(id, name: "bash", input: input)])
            await store.readOutput(issue: "LOO-293", query: RegistryQuery { _, _ in data })
        }
        #expect(store.output.first?.rows.map(\.id.item) == ["b", "c"])
        #expect(store.outputWindowTrimmed)
        let result = try page([
            tool("result-a", call: "a", name: "tool_result", output: "A finished after its call left the window"),
            tool("result-b", call: "b", name: "tool_result", output: "B finished with its call still loaded")
        ])
        await store.readOutput(issue: "LOO-293", query: RegistryQuery { _, _ in result })
        let rows = try #require(store.output.first?.rows)
        #expect(rows.first?.title == "bash · completed")
        #expect(rows.first?.text.hasSuffix("B finished with its call still loaded") == true)
        #expect(rows.last?.title.contains("earlier call not loaded") == true)
        #expect(rows.last?.text.contains("A finished") == true)

        // Growing revisions and a single oversized invisible record must not
        // erase the smaller output that remains useful to the reader.
        let huge = String(repeating: "z", count: 17 * 1_024 * 1_024)
        for id in ["b", "c"] {
            let data = try page([tool(id, name: "bash", input: huge, revision: "2")])
            await store.readOutput(issue: "LOO-293", query: RegistryQuery { _, _ in data })
        }
        let invisible = try page([["source_item_id": "usage", "revision": "1", "event": [
            "type": "usage_checkpoint", "turn_id": "turn", "usage": ["evidence": huge], "final_receipt": false
        ]]])
        await store.readOutput(issue: "LOO-293", query: RegistryQuery { _, _ in invisible })
        #expect(store.output.first?.rows.count == 2)
        #expect(store.output.first?.rows.allSatisfy { $0.title.contains("earlier call not loaded") } == true)
        #expect(store.output.first?.retainedRecords.count == 2)
        #expect(store.output.first?.source.records.isEmpty == true)
        #expect(store.latestOutputRow?.row.item == "b")

        if let path = ProcessInfo.processInfo.environment["LF_WATCH_WINDOW_RENDER_PATH"] {
            let host = NSHostingView(rootView: TaskWatchOutputView(
                store: store, onHistory: { _ in }, onFollow: {}, onReload: {}
            ).frame(width: 1000, height: 650)
                .foregroundStyle(LoopflowPalette.light.text)
                .background(LoopflowPalette.light.background)
                .environment(\.palette, LoopflowPalette.light))
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1000, height: 650),
                                  styleMask: [.titled], backing: .buffered, defer: false)
            window.contentView = host
            host.layoutSubtreeIfNeeded()
            window.displayIfNeeded()
            let bitmap = try #require(host.bitmapImageRepForCachingDisplay(in: host.bounds))
            host.cacheDisplay(in: host.bounds, to: bitmap)
            try #require(bitmap.representation(using: .png, properties: [:])).write(to: URL(fileURLWithPath: path))
            window.contentView = nil
        }
    }

    private func tool(_ id: String, call: String? = nil, name: String, input: String? = nil,
                      output: String? = nil, revision: String = "1") -> [String: Any] {
        ["source_item_id": id, "revision": revision, "event": [
            "type": "item_completed", "turn_id": call == nil ? "assistant" : "user",
            "item": ["type": "tool", "id": call ?? id, "name": name,
                     "status": output == nil ? "running" : "completed",
                     "input": input as Any? ?? NSNull(), "output": output as Any? ?? NSNull()]
        ]]
    }

    private func page(_ records: [[String: Any]]) throws -> String {
        String(decoding: try JSONSerialization.data(withJSONObject: [
            "task_id": "task-watch", "gaps": [], "next_cursor": "cursor", "sources": [[
                "run_id": "run", "provider": "claude", "source": "claude", "stage": NSNull(),
                "records": records, "available": true, "has_more": false, "reset": false, "gaps": []
            ]]
        ]), as: UTF8.self)
    }
}
#endif
