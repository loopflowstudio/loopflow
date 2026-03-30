import Testing
import LoopflowCore
@testable import Concerto

@Suite("Roadmap ordering")
struct RoadmapOrderingTests {
    @Test("planned items stay ahead of shipped items")
    func keepsShippedItemsAtBottom() {
        let items = [
            RoadmapItem(
                id: "shipped-high",
                number: 1,
                title: "Shipped High",
                slug: "shipped-high",
                fileName: "01-shipped-high.md",
                priority: .high,
                isShipped: true
            ),
            RoadmapItem(
                id: "planned-low",
                number: 2,
                title: "Planned Low",
                slug: "planned-low",
                fileName: "02-planned-low.md",
                priority: .low,
                isShipped: false
            ),
            RoadmapItem(
                id: "planned-urgent",
                number: 3,
                title: "Planned Urgent",
                slug: "planned-urgent",
                fileName: "03-planned-urgent.md",
                priority: .urgent,
                isShipped: false
            ),
            RoadmapItem(
                id: "shipped-medium",
                number: 4,
                title: "Shipped Medium",
                slug: "shipped-medium",
                fileName: "04-shipped-medium.md",
                priority: .medium,
                isShipped: true
            )
        ]

        #expect(sortedRoadmapItems(items).map(\.id) == [
            "planned-low",
            "planned-urgent",
            "shipped-high",
            "shipped-medium"
        ])
    }
}
