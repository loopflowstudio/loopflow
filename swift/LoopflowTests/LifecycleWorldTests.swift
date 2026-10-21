#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

/// The Wave inventory and sidebar read a `lifecycle` world built by lf-world and
/// served by the source CLI; no row is written by hand.
@Suite("Lifecycle world", .serialized)
@MainActor
struct LifecycleWorldTests {
    @Test func waveInventoryShowsCurrentTasksAndHistoryFiltersCompletionsByRange() async throws {
        let world = try await BuiltWorld(recipe: "lifecycle")
        defer { world.remove() }
        let model = await world.model()
        let tasks = try #require(model.visibleRoadmaps.first { $0.wave.name == "release" }).tasks.items
        let names = { (group: [RoadmapTask]) in Set(group.map(\.task.name)) }

        let current = WaveTaskGroups(tasks)
        // Owned work survives an upstream cancel until someone disposes of it.
        #expect(names(current.owned) == ["Paused mid-flight", "Canceled upstream", "Lost checkout", "In progress"])
        #expect(names(current.unowned) == ["Not started"])

        // Project history admits successful completions by range, never abandoned or canceled work.
        var filter = TaskHistoryFilter()
        filter.showCompleted = true
        let settled = tasks.filter { !$0.task.isCurrent($0.condition) }
        for (days, expected) in [("7", ["Shipped yesterday", "Shipped this week"]),
                                 ("30", ["Shipped yesterday", "Shipped this week", "Shipped last fortnight"]),
                                 ("0", ["Shipped yesterday", "Shipped this week", "Shipped last fortnight", "Shipped last quarter"])] {
            filter.editDays(days)
            let admitted = settled.filter { filter.includes($0.task, condition: $0.condition, now: Date()) }
            #expect(names(admitted) == Set(expected), "range \(days)")
        }

        #expect(Set(model.visibleWork.standaloneTasks.map(\.task.task.name)) == ["Standalone idea", "Standalone fix"])
    }

    @Test func sidebarRetainsSettledTasksAndHidesPausedOnes() async throws {
        let world = try await BuiltWorld(recipe: "lifecycle")
        defer { world.remove() }
        let model = await world.model()
        let tasks = try #require(model.visibleRoadmaps.first { $0.wave.name == "release" }).tasks.items
        let id = { (name: String) in try #require(tasks.first { $0.task.name == name }?.id) }
        let retained = [try id("Shipped yesterday"), try id("Abandoned attempt"), try id("In progress")]
        let paused = try id("Paused mid-flight")
        for task in retained + [paused] { model.select(.task(id: task)) }

        #expect(Set(retained).isSubset(of: model.workingTaskIDs))
        #expect(!model.workingTaskIDs.contains(paused))
    }
}
#endif
