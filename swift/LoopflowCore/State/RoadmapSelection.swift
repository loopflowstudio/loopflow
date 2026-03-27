import Foundation

@MainActor
@Observable
public final class RoadmapSelection {
    public var selectedItemId: String?
    public var selectedWaveId: String?

    public init(selectedItemId: String? = nil, selectedWaveId: String? = nil) {
        self.selectedItemId = selectedItemId
        self.selectedWaveId = selectedWaveId
    }

    public func select(itemId: String?, waveId: String) {
        selectedItemId = itemId
        selectedWaveId = waveId
    }
}
