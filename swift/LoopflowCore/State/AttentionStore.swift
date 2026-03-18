import Foundation

@MainActor
@Observable
public final class AttentionStore {
    public private(set) var items: [String: AttentionItem] = [:] {
        didSet { recompute() }
    }
    public private(set) var ordered: [AttentionItem] = []

    public init() {}

    public func set(_ item: AttentionItem) {
        items[item.id] = item
    }

    public func setAll(_ newItems: [AttentionItem]) {
        items = Dictionary(uniqueKeysWithValues: newItems.map { ($0.id, $0) })
    }

    public func remove(_ id: String) {
        items.removeValue(forKey: id)
    }

    public func removeAll() {
        items = [:]
    }

    public func item(for id: String) -> AttentionItem? {
        items[id]
    }

    private func recompute() {
        ordered = items.values
            .filter { $0.status != .resolved }
            .sorted {
                let lhs = ($0.status.sortWeight, $0.kind.sortWeight, $0.surfacedAt)
                let rhs = ($1.status.sortWeight, $1.kind.sortWeight, $1.surfacedAt)
                if lhs.0 != rhs.0 { return lhs.0 < rhs.0 }
                if lhs.1 != rhs.1 { return lhs.1 < rhs.1 }
                return lhs.2 < rhs.2
            }
    }
}

private extension AttentionStatus {
    var sortWeight: Int {
        switch self {
        case .surfaced: return 0
        case .viewed: return 1
        case .resolved: return 2
        }
    }
}

private extension AttentionKind {
    var sortWeight: Int {
        switch self {
        case .algedonic: return 0
        case .interactive: return 1
        }
    }
}
