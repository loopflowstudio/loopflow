import Foundation
import Loopflow
import SwiftUI
import ViewInspector
import XCTest
@testable import LoopflowiOS

@MainActor
final class MobileSessionActivityTests: XCTestCase {
    func testActivityUpdatesNeverChangeControlAndDisconnectClearsClaims() async throws {
        let client = SessionActivityClient()
        client.status = report(.working)
        let tasks = try await client.workingTasks()
        let task = try XCTUnwrap(tasks.first)
        let workspace = MobileWorkspace(client: client)
        defer { workspace.leave() }
        await workspace.open(task)
        try await wait { workspace.programStatus?.summary?.state == .working && workspace.state == .viewing }
        workspace.requestControl()
        try await wait { workspace.state == .controlling }
        let first = try XCTUnwrap(client.attachments.first)
        first.reportStatus(report(.blocked))
        try await wait { workspace.programStatus?.summary?.state == .blocked }
        let view = MobileSessionView(task: task, workspace: workspace, theme: .light, chosenSession: .constant(nil))
        XCTAssertNoThrow(try view.inspect().find(text: "Codex · Blocked · Permission"))
        XCTAssertEqual(workspace.state, .controlling)
        first.reportStatus(report(nil))
        try await wait { workspace.programStatus?.summary == nil }
        XCTAssertEqual(workspace.state, .controlling, "Cleared activity must not revoke the keyboard")
        first.reportStatus(report(.working))
        try await wait { workspace.programStatus?.summary != nil }
        workspace.suspend()
        XCTAssertNil(workspace.programStatus)
        first.reportStatus(report(.blocked))
        await workspace.restoreForegroundSession()
        try await wait { workspace.programStatus?.summary?.state == .working }
        XCTAssertEqual(workspace.state, .viewing, "Activity recovery must remain passive")
    }

    func testOldAttachmentCannotReplaceTheNewSessionsActivity() async throws {
        let client = SessionActivityClient()
        client.status = report(.working)
        let tasks = try await client.workingTasks()
        let workspace = MobileWorkspace(client: client)
        defer { workspace.leave() }
        await workspace.open(tasks[0])
        try await wait { workspace.programStatus?.summary?.state == .working }
        let first = try XCTUnwrap(client.attachments.first)
        client.status = report(.blocked)
        await workspace.open(tasks[1])
        try await wait { workspace.programStatus?.summary?.state == .blocked }
        first.reportStatus(report(.working))
        await Task.yield()
        XCTAssertEqual(workspace.programStatus?.summary?.state, .blocked)
        workspace.leave()
        XCTAssertNil(workspace.programStatus)
    }

    private func report(_ state: ProgramStatusState?) -> ProgramStatusRecords {
        ProgramStatusRecords(seen: true, records: state.map {
            [ProgramStatusReport(state: $0, id: "provider", kind: $0 == .blocked ? .permission : nil,
                progress: nil, app: "codex", title: "Review a proposed command",
                msg: "The program is waiting for a response in the terminal.")]
        } ?? [])
    }

    private func wait(_ condition: () -> Bool) async throws {
        let deadline = ContinuousClock.now.advanced(by: .seconds(3))
        while !condition(), ContinuousClock.now < deadline { await Task.yield() }
        guard condition() else { throw RegistryQueryError("Session activity did not reach its expected state") }
    }
}

@MainActor
private final class SessionActivityClient: MobileClient {
    private let fixture = FixtureClient()
    var status = ProgramStatusRecords(seen: false, records: [])
    var attachments: [FixtureAttachment] = []
    func workingTasks() async throws -> [WorkViewPlan.Content] { try await fixture.workingTasks() }
    func observeTasks() async throws -> any MobileTaskObservation { try await fixture.observeTasks() }
    func attach(sessionID: String) async throws -> any MobileAttachment {
        let attached = try await fixture.attach(sessionID: sessionID)
        let attachment = try XCTUnwrap(attached as? FixtureAttachment)
        attachments.append(attachment)
        attachment.reportStatus(status)
        return attachment
    }
}
