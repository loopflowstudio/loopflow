import Loopflow
import SwiftUI
import UIKit
import XCTest
@testable import LoopflowiOS

@MainActor
final class MobileSessionListTests: XCTestCase {
    func testSessionPickerLayouts() async throws {
        bootstrapLoopflowApp()
        let client = FixtureClient()
        let task = try await client.workingTasks()[0]
        let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
        let previous = scene.keyWindow
        for (name, theme, size) in [("light", ThemeDefinition.light, DynamicTypeSize.large),
                                    ("dark", .dark, .large), ("large-text", .light, .accessibility3)] {
            let view = MobileSessionsView(client: client, task: task, currentID: task.primarySession,
                                          theme: theme, choose: { _ in })
                .environment(\.dynamicTypeSize, size)
                .preferredColorScheme(theme.isDark ? .dark : .light)
            let host = UIHostingController(rootView: view)
            let window = UIWindow(windowScene: scene)
            window.rootViewController = host
            window.makeKeyAndVisible()
            defer { window.isHidden = true; previous?.makeKey() }
            try await Task.sleep(for: .milliseconds(300))
            let image = UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
                window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
            }
            let attachment = XCTAttachment(image: image)
            attachment.name = "session-picker-\(name)"
            attachment.lifetime = .keepAlways
            add(attachment)
        }
    }

    func testPagesPreserveRowsThroughFailureAndRefreshReplacesThem() async throws {
        let task = try await FixtureClient().workingTasks()[0]
        let records = try await FixtureClient().sessions(for: task, after: nil).entries
        let client = SessionListClient()
        client.page = try page([records[0]], next: "next-page")
        let model = MobileSessionList(client: client, task: task)
        await model.refresh()
        XCTAssertEqual(model.entries.map(\.id), [records[0].id])
        client.failure = true
        await model.loadMore()
        XCTAssertNotNil(model.error)
        XCTAssertEqual(model.next, "next-page")
        XCTAssertEqual(model.entries.map(\.id), [records[0].id])
        client.failure = false
        client.page = try page(records, next: nil)
        await model.retry()
        XCTAssertEqual(model.entries.map(\.id), records.map(\.id), "Overlapping pages must not duplicate a Session")
        XCTAssertNil(model.next)
        XCTAssertNil(model.error)
        client.page = try page([records[1]], next: nil)
        await model.refresh()
        XCTAssertEqual(model.entries.map(\.id), [records[1].id])
        XCTAssertEqual(client.cursors, [nil, "next-page", "next-page", nil])
    }

    func testDismissedAndOlderReadsCannotOverwriteTheSheet() async throws {
        let task = try await FixtureClient().workingTasks()[0]
        let records = try await FixtureClient().sessions(for: task, after: nil).entries
        let client = SessionListClient()
        client.page = try page([records[0]], next: nil)
        let model = MobileSessionList(client: client, task: task)
        client.delay = true
        let old = Task { await model.refresh() }
        while client.pending == nil { await Task.yield() }
        client.delay = false
        client.page = try page([records[1]], next: nil)
        await model.refresh()
        client.pending?.resume()
        await old.value
        XCTAssertEqual(model.entries.map(\.id), [records[1].id])
        XCTAssertEqual(client.cancellations, 1)
        client.pending = nil
        client.delay = true
        let dismissed = Task { await model.refresh() }
        while client.pending == nil { await Task.yield() }
        model.cancel()
        client.pending?.resume()
        await dismissed.value
        XCTAssertFalse(model.isLoading)
        XCTAssertEqual(model.entries.map(\.id), [records[1].id])
        XCTAssertEqual(client.cancellations, 2)
    }

    func testSessionPageRequestUsesTheSharedTaskScopedFixture() throws {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "session_page", withExtension: "json", subdirectory: "machine"))
        let request = MachineRequest(version: 1, id: "sessions-1", operation: .read(.sessionPage(
            directory: "/repo", headless: false, after: nil, task: "task_00000000000000000000000000000001")))
        XCTAssertEqual(try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(request)),
                       try JSONDecoder().decode(JSONValue.self, from: Data(contentsOf: url)))
    }

    private func page(_ entries: [SessionRecord], next: String?) throws -> SessionPage {
        let rows = try JSONSerialization.jsonObject(with: JSONEncoder().encode(entries))
        let data = try JSONSerialization.data(withJSONObject: ["entries": rows, "next": next as Any? ?? NSNull()])
        return try JSONDecoder().decode(SessionPage.self, from: data)
    }
}

@MainActor
private final class SessionListClient: MobileSessionListingClient {
    var page: SessionPage!
    var failure = false
    var delay = false
    var pending: CheckedContinuation<Void, Never>?
    var cancellations = 0
    var cursors: [String?] = []

    func sessions(for task: WorkViewPlan.Content, after: String?) async throws -> SessionPage {
        cursors.append(after)
        defer { if Task.isCancelled { cancellations += 1 } }
        let result = page!
        if delay { await withCheckedContinuation { pending = $0 } }
        if failure { throw RegistryQueryError("Machine is offline") }
        return result
    }
}
