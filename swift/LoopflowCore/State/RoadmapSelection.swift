import Foundation

@MainActor
@Observable
public final class RoadmapSelection {
    public var selectedItemId: String?

    public init(selectedItemId: String? = nil) {
        self.selectedItemId = selectedItemId
    }

    public func select(itemId: String?) {
        selectedItemId = itemId
    }
}
