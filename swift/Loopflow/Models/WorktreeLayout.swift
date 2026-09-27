import Foundation
import Observation

/// Worktree slots surround complete terminal layouts. Neither tree contains
/// leaves belonging to the other level.
public indirect enum WorktreeLayout: Equatable {
    case leaf(id: String, path: String?)
    case split(SplitAxis, first: WorktreeLayout, second: WorktreeLayout)

    public var slots: [(id: String, path: String?)] {
        switch self {
        case .leaf(let id, let path): [(id, path)]
        case .split(_, let first, let second): first.slots + second.slots
        }
    }

    func replacing(_ id: String, with replacement: WorktreeLayout) -> WorktreeLayout {
        switch self {
        case .leaf(let slot, _): slot == id ? replacement : self
        case .split(let axis, let first, let second):
            .split(axis, first: first.replacing(id, with: replacement), second: second.replacing(id, with: replacement))
        }
    }

    func removing(_ id: String) -> WorktreeLayout? {
        switch self {
        case .leaf(let slot, _): return slot == id ? nil : self
        case .split(let axis, let first, let second):
            guard let left = first.removing(id) else { return second }
            guard let right = second.removing(id) else { return first }
            return .split(axis, first: left, second: right)
        }
    }
}

@MainActor
@Observable
public final class WorktreeLayoutStore {
    public private(set) var layout: WorktreeLayout
    public private(set) var focusedSlotId: String
    public private(set) var knownPaths: Set<String>

    public init(path: String) {
        let id = UUID().uuidString
        layout = .leaf(id: id, path: path)
        focusedSlotId = id
        knownPaths = [path]
    }

    public var focusedPath: String? {
        layout.slots.first { $0.id == focusedSlotId }?.path
    }

    public func focus(_ id: String) {
        guard layout.slots.contains(where: { $0.id == id }) else { return }
        focusedSlotId = id
    }

    public func select(_ path: String, in slotId: String? = nil) {
        knownPaths.insert(path)
        if let existing = layout.slots.first(where: { $0.path == path }) {
            focusedSlotId = existing.id
            return
        }
        let target = slotId ?? focusedSlotId
        guard layout.slots.contains(where: { $0.id == target }) else { return }
        layout = layout.replacing(target, with: .leaf(id: target, path: path))
        focusedSlotId = target
    }

    public func split(_ id: String, axis: SplitAxis) {
        guard let slot = layout.slots.first(where: { $0.id == id }) else { return }
        let newId = UUID().uuidString
        layout = layout.replacing(id, with: .split(
            axis, first: .leaf(id: id, path: slot.path), second: .leaf(id: newId, path: nil)
        ))
        focusedSlotId = newId
    }

    public func close(_ id: String) {
        guard layout.slots.contains(where: { $0.id == id }) else { return }
        layout = layout.removing(id) ?? .leaf(id: id, path: nil)
        if !layout.slots.contains(where: { $0.id == focusedSlotId }), let first = layout.slots.first {
            focusedSlotId = first.id
        }
    }
}
