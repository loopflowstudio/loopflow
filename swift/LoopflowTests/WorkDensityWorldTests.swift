#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

/// Reads a `density` world built by `lf-world` through the built CLI.
@Suite("Sidebar density on a built world", .serialized)
@MainActor
struct WorkDensityWorldTests {
    @Test("At density the sidebar lists opened started Tasks and one row per Wave, never orphan Sessions")
    func sidebarDensity() async throws {
        let world = try await BuiltWorld(recipe: "density")
        defer { world.remove() }
        let model = await world.model()

        let roadmaps = model.visibleRoadmaps
        #expect(roadmaps.map(\.wave.name).sorted() == ["infrastructure", "intelligence", "product"])
        #expect(roadmaps.allSatisfy { $0.tasks.items.count == 50 })
        let product = try #require(roadmaps.first { $0.wave.name == "product" })
        let started = product.tasks.items.filter { $0.runtime?.owner != nil }
        #expect(started.count == 8)
        #expect(model.projection.unmatchedSessions.count == 4)
        let orphans = model.projection.unmatchedSessions.map(\.id)

        // Opening each started product Task retains it in the working list.
        for task in started { model.select(.task(id: task.id)) }
        let seventh = try #require(started.first { $0.task.name.hasSuffix(" 7") })
        model.select(.task(id: seventh.id))
        let outline = try WorkNavigator(model: model, onOpenSession: { _ in }).inspect()
        #expect(outline.findAll(ViewType.Button.self) {
            (try? $0.accessibilityIdentifier().hasPrefix("work-task-")) == true
        }.count == 8)
        for roadmap in roadmaps {
            _ = try outline.find(viewWithAccessibilityIdentifier: "work-wave-\(roadmap.wave.id)")
        }
        #expect(throws: (any Error).self) { try outline.find(WaveLensView.self) }
        for orphan in orphans {
            #expect(throws: (any Error).self) { try outline.find(viewWithAccessibilityIdentifier: "session-row-\(orphan)") }
        }

        // Binding every orphan leaves no unmatched Session behind.
        for orphan in orphans {
            _ = try await world.query.bindSession(id: orphan, taskId: seventh.task.identifier, cwd: world.repo.path)
        }
        await model.refreshSessions()
        #expect(model.projection.unmatchedSessions.isEmpty)
        #expect(model.taskSessions(seventh.id).count == 5)
    }
}
#endif
